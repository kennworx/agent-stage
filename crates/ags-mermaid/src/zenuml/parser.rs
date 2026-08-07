//! Reading `zenuml` source.
//!
//! ```text
//! zenuml
//!   @Actor User
//!   participant Server as API
//!   User->Server: poll
//!   Server.handle() {
//!     Store.read()
//!     return rows
//!   }
//!   loop (3 times) { … } / alt (ok) { … } else { … }
//! ```
//!
//! The one thing that is not a straight read of the text: a method call implies
//! a return the source never writes. `Store.read()` on its own is a call *and* a
//! dashed reply; the same call with a block is a call whose reply waits until the
//! block closes, and is dropped entirely if an explicit `return` already covered
//! it. That bookkeeping is what the frame stack is for.
//!
//! Matching is hand-rolled rather than regex-driven, as everywhere else in this
//! crate — see `text.rs` for why.

use std::collections::HashSet;

use super::types::{
    ArrowHead, Diagram, Fragment, FragmentKind, LineStyle, Message, MessageKind, Participant,
    Section,
};

use super::lex::{
    after_word, annotator_declaration, arrow_message, declaration, head, head_of, is_bare_name,
    method_call, normalize_keyword, strip_parens, strip_trailing_brace, DIVIDERS, OPENERS,
};

/// What a frame on the stack is holding open.
#[derive(Debug, Clone)]
enum Body {
    /// A block that neither replies nor draws a box: `{ … }` on its own.
    Plain,
    /// A `X.method() { … }` block. Its reply runs from `source` back to `target`
    /// when the block closes, unless an explicit `return` already covered it.
    Call {
        source: String,
        target: String,
        returned: bool,
    },
    /// A control-flow block, which becomes a box once it closes.
    Fragment(Fragment),
}

/// One open block, and who is speaking inside it.
#[derive(Debug, Clone)]
struct Frame {
    sender: String,
    caller: String,
    body: Body,
}

/// The reading state: the diagram so far, and the blocks still open.
#[derive(Debug, Default)]
struct Reader {
    diagram: Diagram,
    ids: HashSet<String>,
    frames: Vec<Frame>,
    /// A fragment frame closed by a `}` that a following `else` may reopen.
    /// Finalized lazily, because until the next line is read there is no telling
    /// which of the two it was.
    pending: Option<Frame>,
}

impl Reader {
    /// The participant a message with no context comes from.
    fn root(&self) -> String {
        self.diagram
            .participants
            .first()
            .map_or_else(String::new, |p| p.id.clone())
    }

    /// The innermost open block, or a notional one rooted at the first
    /// participant when nothing is open.
    fn current(&self) -> Frame {
        self.frames.last().cloned().unwrap_or_else(|| {
            let root = self.root();
            Frame {
                sender: root.clone(),
                caller: root,
                body: Body::Plain,
            }
        })
    }

    /// Open a block that only balances a brace.
    fn push_plain(&mut self) {
        let frame = self.current();
        self.frames.push(Frame {
            sender: frame.sender,
            caller: frame.caller,
            body: Body::Plain,
        });
    }

    fn fragment_depth(&self) -> usize {
        self.frames
            .iter()
            .filter(|frame| matches!(frame.body, Body::Fragment(_)))
            .count()
    }

    fn push_message(&mut self, message: Message) {
        self.diagram.messages.push(message);
    }

    /// The dashed reply a call gets back.
    fn push_return(&mut self, from: String, to: String, label: String) {
        self.push_message(Message {
            from,
            to,
            label,
            kind: MessageKind::Return,
            line_style: LineStyle::Dashed,
            arrow_head: ArrowHead::Open,
        });
    }

    /// Commit a closed fragment that nothing reopened.
    fn finalize_pending(&mut self) {
        if let Some(Frame {
            body: Body::Fragment(fragment),
            ..
        }) = self.pending.take()
        {
            self.diagram.fragments.push(fragment);
        }
    }

    /// Pop one block, emitting its implicit reply or holding its box.
    fn close_frame(&mut self) {
        self.finalize_pending();
        let Some(Frame {
            sender,
            caller,
            body,
        }) = self.frames.pop()
        else {
            return;
        };
        match body {
            Body::Call {
                source,
                target,
                returned,
            } => {
                if !returned && !source.is_empty() && !target.is_empty() {
                    self.push_return(source, target, String::new());
                }
            }
            Body::Fragment(mut fragment) => {
                fragment.end_index = self.diagram.messages.len();
                self.pending = Some(Frame {
                    sender,
                    caller,
                    body: Body::Fragment(fragment),
                });
            }
            Body::Plain => {}
        }
    }

    /// Note that the nearest enclosing call replied for itself.
    fn mark_call_returned(&mut self) {
        for frame in self.frames.iter_mut().rev() {
            if let Body::Call { returned, .. } = &mut frame.body {
                *returned = true;
                return;
            }
        }
    }

    /// Reopen the held fragment for a continuation section.
    fn reopen_section(&mut self, keyword: String, label: String) {
        let Some(mut frame) = self.pending.take() else {
            return;
        };
        let index = self.diagram.messages.len();
        if let Body::Fragment(fragment) = &mut frame.body {
            fragment.sections.push(Section {
                index,
                keyword,
                label,
            });
        }
        self.frames.push(frame);
    }

    /// Infer a participant a message named but nobody declared.
    fn ensure(&mut self, id: &str) {
        if id.is_empty() || self.ids.contains(id) {
            return;
        }
        self.ids.insert(id.to_string());
        self.diagram.participants.push(Participant {
            id: id.to_string(),
            label: id.to_string(),
            annotator: None,
        });
    }

    /// Declare a participant, or enrich one a message already inferred.
    fn add(&mut self, id: &str, label: &str, annotator: Option<&str>) {
        if !self.ids.contains(id) {
            self.ids.insert(id.to_string());
            self.diagram.participants.push(Participant {
                id: id.to_string(),
                label: label.to_string(),
                annotator: annotator.map(ToString::to_string),
            });
            return;
        }
        if let Some(existing) = self.diagram.participants.iter_mut().find(|p| p.id == id) {
            if !label.is_empty() && label != id && existing.label == id {
                existing.label = label.to_string();
            }
            if existing.annotator.is_none() {
                existing.annotator = annotator.map(ToString::to_string);
            }
        }
    }
}

/// The line handlers. Split from the state so each reads as one rule.
impl Reader {
    /// `} else {` — the brace closes the section, but the fragment stays open to
    /// receive the next one.
    fn continuation(&mut self, line: &str) -> bool {
        let Some(rest) = line.strip_prefix('}') else {
            return false;
        };
        let Some((keyword, tail)) = head_of(rest.trim_start(), &DIVIDERS) else {
            return false;
        };
        if self.pending.is_none() {
            if !matches!(self.frames.last().map(|f| &f.body), Some(Body::Fragment(_))) {
                return false;
            }
            self.pending = self.frames.pop();
        }
        self.reopen_section(
            normalize_keyword(keyword),
            strip_trailing_brace(tail).to_string(),
        );
        true
    }

    /// Pop a block per leading `}`. `None` once nothing is left on the line.
    fn close_braces<'a>(&mut self, line: &'a str) -> Option<&'a str> {
        let mut line = line;
        while let Some(rest) = line.strip_prefix('}') {
            self.close_frame();
            line = rest.trim();
            if line.is_empty() {
                break;
            }
        }
        (!line.is_empty()).then_some(line)
    }

    /// A standalone `else {` reopening a fragment the previous line closed.
    fn divider(&mut self, content: &str) -> bool {
        if self.pending.is_none() {
            return false;
        }
        let Some((keyword, tail)) = head_of(content, &DIVIDERS) else {
            return false;
        };
        self.reopen_section(
            normalize_keyword(keyword),
            strip_parens(tail.trim()).to_string(),
        );
        true
    }

    /// `return value` / `@return value`, drawn back to whoever called in.
    fn returns(&mut self, content: &str) -> bool {
        let body = content.strip_prefix('@').unwrap_or(content);
        let Some(tail) = head(body, "return") else {
            return false;
        };
        let frame = self.current();
        self.ensure(&frame.sender);
        self.ensure(&frame.caller);
        self.push_return(frame.sender, frame.caller, tail.trim().to_string());
        self.mark_call_returned();
        true
    }

    fn open_fragment(&mut self, keyword: &str, label: &str) {
        let index = self.diagram.messages.len();
        let depth = self.fragment_depth();
        let frame = self.current();
        self.frames.push(Frame {
            sender: frame.sender,
            caller: frame.caller,
            body: Body::Fragment(Fragment {
                kind: FragmentKind::from_keyword(keyword),
                label: label.to_string(),
                start_index: index,
                end_index: index,
                sections: Vec::new(),
                depth,
            }),
        });
    }

    fn participant(&mut self, content: &str) -> bool {
        let Some(tail) = after_word(content, "participant") else {
            return false;
        };
        let Some((id, label)) = declaration(tail, true) else {
            return false;
        };
        self.add(&id, &label, None);
        true
    }

    fn annotator(&mut self, content: &str) -> bool {
        let Some((annotator, id, label)) = annotator_declaration(content) else {
            return false;
        };
        self.add(&id, &label, Some(&annotator));
        true
    }

    fn arrow(&mut self, content: &str) -> bool {
        let Some((from, to, label, line_style)) = arrow_message(content) else {
            return false;
        };
        self.ensure(&from);
        self.ensure(&to);
        self.push_message(Message {
            from,
            to,
            label,
            kind: MessageKind::Sync,
            line_style,
            arrow_head: ArrowHead::Filled,
        });
        true
    }

    /// A method call, and the reply it implies.
    ///
    /// With a block the reply waits for the closing brace, so anything the callee
    /// does in the meantime is drawn between the two. Without one there is
    /// nothing to wait for, and the reply follows immediately.
    fn calls(&mut self, content: &str, opens: bool) -> bool {
        let Some((to, label)) = method_call(content) else {
            return false;
        };
        let sender = self.current().sender;
        let from = if sender.is_empty() {
            to.to_string()
        } else {
            sender
        };
        self.ensure(&from);
        self.ensure(to);
        self.push_message(Message {
            from: from.clone(),
            to: to.to_string(),
            label: label.to_string(),
            kind: MessageKind::Sync,
            line_style: LineStyle::Solid,
            arrow_head: ArrowHead::Filled,
        });
        if opens {
            self.frames.push(Frame {
                sender: to.to_string(),
                caller: from.clone(),
                body: Body::Call {
                    source: to.to_string(),
                    target: from,
                    returned: false,
                },
            });
        } else {
            self.push_return(to.to_string(), from, String::new());
        }
        true
    }

    /// Everything a line can be, in the order the reference tries them.
    fn statement(&mut self, content: &str, opens: bool) {
        if self.returns(content) {
            return;
        }
        if let Some((keyword, tail)) = head_of(content, &OPENERS) {
            // A control keyword with no block opens nothing, and the header line
            // is dropped rather than read as a participant named `loop`.
            if opens {
                self.open_fragment(keyword, strip_parens(tail.trim()));
            }
            return;
        }
        // A call with a block pushes its own frame; every other statement that
        // opens one only has a brace to balance.
        if self.calls_or_declares(content, opens) {
            return;
        }
        if opens {
            self.push_plain();
        }
    }

    /// The statements that name or address a participant.
    fn calls_or_declares(&mut self, content: &str, opens: bool) -> bool {
        if self.participant(content) || self.annotator(content) || self.arrow(content) {
            if opens {
                self.push_plain();
            }
            return true;
        }
        if self.calls(content, opens) {
            return true;
        }
        if is_bare_name(content) {
            self.add(content, content, None);
            if opens {
                self.push_plain();
            }
            return true;
        }
        false
    }

    /// Read one line.
    fn line(&mut self, index: usize, line: &str) {
        if head(line, "zenuml").is_some() && (index == 0 || line.eq_ignore_ascii_case("zenuml")) {
            return;
        }
        if self.continuation(line) {
            return;
        }
        let Some(line) = self.close_braces(line) else {
            return;
        };
        let (body, opens) = line
            .strip_suffix('{')
            .map_or((line, false), |body| (body, true));
        let content = body.trim();
        if content.is_empty() {
            self.finalize_pending();
            if opens {
                self.push_plain();
            }
            return;
        }
        if self.divider(content) {
            return;
        }
        // Any other content settles a fragment the previous line closed.
        self.finalize_pending();
        self.statement(content, opens);
    }
}

/// Parse a `ZenUML` source.
pub fn parse(source: &str) -> Diagram {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("%%"))
        .collect();
    let mut reader = Reader::default();
    for (index, line) in lines.iter().enumerate() {
        reader.line(index, line);
    }
    // Close whatever unbalanced input left open, so a truncated source still
    // draws the blocks it did manage to open.
    while !reader.frames.is_empty() {
        reader.close_frame();
    }
    reader.finalize_pending();
    reader.diagram
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(diagram: &Diagram) -> Vec<&str> {
        diagram
            .participants
            .iter()
            .map(|p| p.id.as_str())
            .collect::<Vec<&str>>()
    }

    fn wires(diagram: &Diagram) -> Vec<(String, String, String)> {
        diagram
            .messages
            .iter()
            .map(|m| (m.from.clone(), m.to.clone(), m.label.clone()))
            .collect()
    }

    #[test]
    fn the_header_is_not_a_participant() {
        let out = parse("zenuml\nA->B: hi");
        assert_eq!(ids(&out), ["A", "B"]);
    }

    #[test]
    fn a_later_header_line_is_skipped_only_when_it_is_bare() {
        // `zenuml` alone repeats the header; `zenumlish` is just a name.
        assert_eq!(ids(&parse("zenuml\nzenuml\nA->B: x")), ["A", "B"]);
        assert_eq!(ids(&parse("zenuml\nzenumlish")), ["zenumlish"]);
    }

    #[test]
    fn a_declaration_names_and_labels_a_participant() {
        let out = parse("zenuml\nparticipant A as Alice\nparticipant B");
        assert_eq!(ids(&out), ["A", "B"]);
        assert_eq!(out.participants[0].label, "Alice");
        assert_eq!(out.participants[1].label, "B");
    }

    #[test]
    fn a_quoted_name_and_label_lose_their_quotes() {
        let out = parse("zenuml\nparticipant \"A\" as \"Alice B\"");
        assert_eq!(ids(&out), ["A"]);
        assert_eq!(out.participants[0].label, "Alice B");
    }

    #[test]
    fn a_declaration_that_is_not_one_falls_through_to_the_other_rules() {
        // `participant A B` has no `as`, so the reference's pattern fails and the
        // line is read as something else — here, nothing at all.
        let out = parse("zenuml\nparticipant A B");
        assert!(out.participants.is_empty());
        assert!(out.messages.is_empty());
    }

    #[test]
    fn an_annotator_declares_a_participant_with_a_stereotype() {
        let out = parse("zenuml\n@Actor User\n@Database Store as Records");
        assert_eq!(out.participants[0].annotator.as_deref(), Some("Actor"));
        assert_eq!(out.participants[1].label, "Records");
        assert_eq!(out.participants[1].annotator.as_deref(), Some("Database"));
    }

    #[test]
    fn an_annotator_alias_is_spelled_in_lower_case_only() {
        // The reference's annotator rule carries no `/i`, so `AS` is a name.
        let out = parse("zenuml\n@Actor User AS Someone");
        assert!(out.participants.is_empty());
    }

    #[test]
    fn an_annotator_enriches_a_participant_a_message_already_inferred() {
        let out = parse("zenuml\nA->B: hi\n@Actor A as Alice");
        assert_eq!(out.participants[0].label, "Alice");
        assert_eq!(out.participants[0].annotator.as_deref(), Some("Actor"));
    }

    #[test]
    fn a_second_annotator_does_not_displace_the_first() {
        let out = parse("zenuml\n@Actor A\n@Database A");
        assert_eq!(out.participants[0].annotator.as_deref(), Some("Actor"));
    }

    #[test]
    fn a_bare_name_declares_a_participant() {
        let out = parse("zenuml\nDatabase\nA->Database: read");
        assert_eq!(ids(&out), ["Database", "A"]);
    }

    #[test]
    fn every_arrow_spelling_is_read() {
        let out = parse("zenuml\nA->B: one\nA-->B: two\nA->>B: three\nA-->>B: four");
        assert_eq!(out.messages.len(), 4);
        assert_eq!(out.messages[0].line_style, LineStyle::Solid);
        assert_eq!(out.messages[1].line_style, LineStyle::Dashed);
        assert_eq!(out.messages[2].line_style, LineStyle::Solid);
        assert_eq!(out.messages[3].line_style, LineStyle::Dashed);
    }

    #[test]
    fn an_arrow_takes_the_head_of_a_dotted_reference() {
        let out = parse("zenuml\nA.field -> B.other : hi");
        assert_eq!(wires(&out), [("A".into(), "B".into(), "hi".into())]);
    }

    #[test]
    fn an_arrow_without_a_label_is_not_a_message() {
        assert!(parse("zenuml\nA->B:").messages.is_empty());
        assert!(parse("zenuml\nA->B").messages.is_empty());
        assert!(parse("zenuml\nA->: hi").messages.is_empty());
        assert!(parse("zenuml\n->B: hi").messages.is_empty());
        assert!(parse("zenuml\nA-B: hi").messages.is_empty());
    }

    #[test]
    fn a_leaf_call_gets_its_reply_straight_back() {
        let out = parse("zenuml\nA->B: go\nB.work()");
        assert_eq!(
            wires(&out),
            [
                ("A".into(), "B".into(), "go".into()),
                ("A".into(), "B".into(), "work()".into()),
                ("B".into(), "A".into(), String::new()),
            ]
        );
        assert_eq!(out.messages[2].kind, MessageKind::Return);
    }

    #[test]
    fn a_call_with_no_participant_yet_speaks_to_itself() {
        let out = parse("zenuml\nB.work()");
        assert_eq!(wires(&out)[0], ("B".into(), "B".into(), "work()".into()));
    }

    #[test]
    fn a_call_block_defers_its_reply_until_the_block_closes() {
        let out = parse("zenuml\nA->B: go\nB.work() {\nC.help()\n}");
        assert_eq!(
            wires(&out),
            [
                ("A".into(), "B".into(), "go".into()),
                ("A".into(), "B".into(), "work()".into()),
                ("B".into(), "C".into(), "help()".into()),
                ("C".into(), "B".into(), String::new()),
                ("B".into(), "A".into(), String::new()),
            ]
        );
    }

    #[test]
    fn an_explicit_return_replaces_the_implicit_one() {
        let out = parse("zenuml\nA->B: go\nB.work() {\nreturn done\n}");
        assert_eq!(
            wires(&out),
            [
                ("A".into(), "B".into(), "go".into()),
                ("A".into(), "B".into(), "work()".into()),
                ("B".into(), "A".into(), "done".into()),
            ]
        );
    }

    #[test]
    fn an_at_return_is_read_the_same_way() {
        let out = parse("zenuml\nA->B: go\nB.work() {\n@return done\n}");
        assert_eq!(out.messages.last().map(|m| m.label.as_str()), Some("done"));
        assert_eq!(out.messages.len(), 3);
    }

    #[test]
    fn a_return_inside_a_branch_still_answers_for_the_call_around_it() {
        // The search skips past the fragment frame to the call underneath it, so
        // the call does not also emit a reply of its own.
        let out = parse("zenuml\nA->B: go\nB.work() {\nalt (ok) {\nreturn done\n}\n}");
        assert_eq!(
            wires(&out),
            [
                ("A".into(), "B".into(), "go".into()),
                ("A".into(), "B".into(), "work()".into()),
                ("B".into(), "A".into(), "done".into()),
            ]
        );
    }

    #[test]
    fn a_return_outside_any_call_runs_from_the_root_to_itself() {
        let out = parse("zenuml\nA->B: go\nreturn later");
        assert_eq!(wires(&out)[1], ("A".into(), "A".into(), "later".into()));
    }

    #[test]
    fn a_dangling_call_block_still_replies_at_the_end_of_the_source() {
        let out = parse("zenuml\nA->B: go\nB.work() {");
        assert_eq!(out.messages.len(), 3);
        assert_eq!(out.messages[2].kind, MessageKind::Return);
    }

    #[test]
    fn a_fragment_records_the_range_it_wraps() {
        let out = parse("zenuml\nloop (3 times) {\nA->B: poll\n}");
        assert_eq!(out.fragments.len(), 1);
        let fragment = &out.fragments[0];
        assert_eq!(fragment.kind, FragmentKind::Loop);
        assert_eq!(fragment.label, "3 times");
        assert_eq!((fragment.start_index, fragment.end_index), (0, 1));
        assert_eq!(fragment.depth, 0);
    }

    #[test]
    fn a_nested_fragment_is_deeper_than_the_one_around_it() {
        let out = parse("zenuml\nloop (n) {\nalt (ok) {\nA->B: x\n}\n}");
        let depths: Vec<usize> = out.fragments.iter().map(|f| f.depth).collect();
        assert_eq!(depths, [1, 0], "the inner block closes first");
    }

    #[test]
    fn a_continuation_on_the_closing_line_keeps_the_same_box() {
        let out = parse("zenuml\nalt (ok) {\nA->B: yes\n} else {\nA->B: no\n}");
        assert_eq!(out.fragments.len(), 1);
        let fragment = &out.fragments[0];
        assert_eq!(fragment.sections.len(), 1);
        assert_eq!(fragment.sections[0].keyword, "else");
        assert_eq!(fragment.sections[0].index, 1);
        assert_eq!(fragment.end_index, 2);
    }

    #[test]
    fn a_continuation_on_its_own_line_reopens_the_box_too() {
        let out = parse("zenuml\nalt (ok) {\nA->B: yes\n}\nelse (no) {\nA->B: no\n}");
        assert_eq!(out.fragments.len(), 1);
        assert_eq!(out.fragments[0].sections[0].label, "no");
    }

    #[test]
    fn every_continuation_keyword_is_recognised() {
        let source =
            "zenuml\ntry {\nA->B: go\n} catch (e) {\nA->B: oops\n} finally {\nA->B: done\n}";
        let out = parse(source);
        let keywords: Vec<&str> = out.fragments[0]
            .sections
            .iter()
            .map(|s| s.keyword.as_str())
            .collect();
        assert_eq!(keywords, ["catch", "finally"]);
    }

    #[test]
    fn an_else_if_wins_over_a_bare_else() {
        let out = parse("zenuml\nalt (a) {\nA->B: x\n} else   if (b) {\nA->B: y\n}");
        assert_eq!(out.fragments[0].sections[0].keyword, "else if");
        assert_eq!(out.fragments[0].sections[0].label, "b");
    }

    #[test]
    fn a_par_block_takes_its_and_sections() {
        let out = parse("zenuml\npar {\nA->B: x\n} and {\nA->C: y\n}");
        assert_eq!(out.fragments[0].kind, FragmentKind::Par);
        assert_eq!(out.fragments[0].sections[0].keyword, "and");
    }

    #[test]
    fn a_continuation_with_no_box_to_reopen_is_ignored() {
        let out = parse("zenuml\n} else {\nA->B: x\n}");
        assert!(out.fragments.is_empty());
        assert_eq!(out.messages.len(), 1);
    }

    #[test]
    fn a_control_keyword_without_a_block_opens_nothing() {
        let out = parse("zenuml\nloop three times\nA->B: x");
        assert!(out.fragments.is_empty());
        assert_eq!(out.messages.len(), 1);
    }

    #[test]
    fn a_bare_block_only_balances_its_brace() {
        let out = parse("zenuml\nA->B: go\n{\nA->B: inner\n}");
        assert_eq!(out.messages.len(), 2);
        assert!(out.fragments.is_empty());
    }

    #[test]
    fn a_line_that_is_nothing_recognisable_still_balances_a_brace() {
        let out = parse("zenuml\nA->B: go\n123 !! {\nA->B: inner\n}\nA->B: after");
        assert_eq!(out.messages.len(), 3);
    }

    #[test]
    fn several_braces_on_one_line_close_several_blocks() {
        let out = parse("zenuml\nA.one() {\nB.two() {\n} }");
        assert_eq!(out.messages.len(), 4, "two calls and two replies");
    }

    #[test]
    fn a_comment_and_a_blank_line_are_dropped_before_reading() {
        let out = parse("zenuml\n\n%% a note\n  A->B: hi  \n");
        assert_eq!(wires(&out), [("A".into(), "B".into(), "hi".into())]);
    }

    #[test]
    fn a_self_call_names_the_same_participant_twice() {
        let out = parse("zenuml\nA\nA.think()");
        assert_eq!(wires(&out)[0], ("A".into(), "A".into(), "think()".into()));
    }

    #[test]
    fn a_source_of_nothing_parses_to_nothing() {
        let out = parse("zenuml");
        assert_eq!(out, Diagram::default());
    }

    #[test]
    fn a_fragment_that_wraps_no_message_still_survives() {
        let out = parse("zenuml\nA->B: x\nopt (never) {\n}");
        assert_eq!(out.fragments.len(), 1);
        assert_eq!(out.fragments[0].start_index, 1);
        assert_eq!(out.fragments[0].end_index, 1);
    }
}
