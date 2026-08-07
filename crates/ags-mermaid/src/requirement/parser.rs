//! Reading `requirementDiagram` source.
//!
//! ```text
//! requirementDiagram
//!   requirement test_req {
//!     id: 1
//!     text: the test text.
//!     risk: high
//!     verifymethod: test
//!   }
//!   element test_entity {
//!     type: simulation
//!     docref: ./spec.md
//!   }
//!   test_entity - satisfies -> test_req
//!   test_req <- traces - other_entity
//! ```
//!
//! Braces carry the structure, not indentation, so this parser reads trimmed
//! lines and tracks which block it is inside.

use super::types::{Diagram, Element, Kind, Relationship, Requirement};
use crate::keyword::is_word;

/// Strip one leading and one trailing quote character, each independently.
fn unquote(text: &str) -> &str {
    let head = text.strip_prefix(['"', '\'']).unwrap_or(text);
    head.strip_suffix(['"', '\'']).unwrap_or(head)
}

/// Everything before a `%%` comment.
fn strip_comment(line: &str) -> &str {
    line.split("%%").next().unwrap_or(line)
}

/// A `key: value` line inside a block.
fn parse_field(line: &str) -> Option<(String, String)> {
    let (key, value) = line.split_once(':')?;
    let key = key.trim();
    if key.is_empty() || !key.chars().all(is_word) {
        return None;
    }
    let value = unquote(value.trim()).trim();
    (!value.is_empty()).then(|| (key.to_ascii_lowercase(), value.to_string()))
}

/// A `<keyword> <name> {` line.
fn parse_block_open(line: &str) -> Option<(&str, String)> {
    let body = line.strip_suffix('{')?.trim_end();
    let (keyword, name) = body.split_once(char::is_whitespace)?;
    if keyword.is_empty() || !keyword.chars().all(is_word) {
        return None;
    }
    let name = name.trim();
    (!name.is_empty()).then(|| (keyword, name.to_string()))
}

/// A relationship line, in either direction.
fn parse_relationship(line: &str) -> Option<Relationship> {
    // Forward: `src - kind -> dest`. The arrow is looked for first, so a name
    // containing a dash cannot be mistaken for the separator.
    if let Some((head, dest)) = line.split_once("->") {
        let (source, kind) = head.rsplit_once('-')?;
        let (source, kind, dest) = (source.trim(), kind.trim(), dest.trim());
        // `src - kind` needs the leading dash trimmed off `src`'s own tail.
        let source = source.strip_suffix('-').unwrap_or(source).trim();
        if !source.is_empty() && !dest.is_empty() && kind.chars().all(is_word) && !kind.is_empty() {
            return Some(Relationship {
                source: source.to_string(),
                kind: kind.to_string(),
                dest: dest.to_string(),
            });
        }
    }
    // Reverse: `dest <- kind - src`, which means the same thing the other way.
    if let Some((dest, tail)) = line.split_once("<-") {
        let (kind, source) = tail.split_once('-')?;
        let (dest, kind, source) = (dest.trim(), kind.trim(), source.trim());
        if !source.is_empty() && !dest.is_empty() && kind.chars().all(is_word) && !kind.is_empty() {
            return Some(Relationship {
                source: source.to_string(),
                kind: kind.to_string(),
                dest: dest.to_string(),
            });
        }
    }
    None
}

/// Which block the reader is inside.
enum Open {
    None,
    Requirement,
    Element,
}

/// Parse a requirement diagram. A line that matches nothing is skipped.
pub fn parse(source: &str) -> Diagram {
    let mut diagram = Diagram::default();
    let mut open = Open::None;

    for raw in source.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if !matches!(open, Open::None) {
            if line.starts_with('}') {
                open = Open::None;
                continue;
            }
            let Some((key, value)) = parse_field(line) else {
                continue;
            };
            match open {
                Open::Requirement => {
                    if let Some(req) = diagram.requirements.last_mut() {
                        match key.as_str() {
                            "id" => req.id = Some(value),
                            "text" => req.text = Some(value),
                            "risk" => req.risk = Some(value),
                            "verifymethod" => req.verify_method = Some(value),
                            _ => {}
                        }
                    }
                }
                Open::Element => {
                    if let Some(element) = diagram.elements.last_mut() {
                        match key.as_str() {
                            "type" => element.kind = Some(value),
                            "docref" => element.docref = Some(value),
                            _ => {}
                        }
                    }
                }
                Open::None => {}
            }
            continue;
        }
        if let Some((keyword, name)) = parse_block_open(line) {
            if keyword == "element" {
                diagram.elements.push(Element {
                    name,
                    kind: None,
                    docref: None,
                });
                open = Open::Element;
            } else if let Some(kind) = Kind::from_keyword(keyword) {
                diagram.requirements.push(Requirement {
                    name,
                    kind,
                    id: None,
                    text: None,
                    risk: None,
                    verify_method: None,
                });
                open = Open::Requirement;
            }
            // An unrecognised keyword opens nothing, so its body is read as
            // top-level lines rather than silently attached to the last block.
            continue;
        }
        if let Some(relationship) = parse_relationship(line) {
            diagram.relationships.push(relationship);
        }
    }
    diagram
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_needs_a_word_for_a_key_and_something_for_a_value() {
        assert_eq!(
            parse_field("id: 1"),
            Some(("id".to_string(), "1".to_string()))
        );
        assert_eq!(
            parse_field("Text: \"under 50ms\""),
            Some(("text".to_string(), "under 50ms".to_string())),
            "the key is folded and the value unquoted"
        );
        assert_eq!(parse_field("no colon here"), None);
        assert_eq!(parse_field(": 1"), None, "no key");
        assert_eq!(parse_field("id:  "), None, "no value");
        assert_eq!(parse_field("two words: 1"), None, "a key is one word");
    }

    #[test]
    fn a_block_opens_on_a_keyword_and_a_name() {
        assert_eq!(
            parse_block_open("requirement speed {"),
            Some(("requirement", "speed".to_string()))
        );
        assert_eq!(
            parse_block_open("element  suite  {"),
            Some(("element", "suite".to_string()))
        );
        assert_eq!(parse_block_open("requirement speed"), None, "no brace");
        assert_eq!(parse_block_open("requirement {"), None, "no name");
        assert_eq!(
            parse_block_open("a-b name {"),
            None,
            "a keyword is one word"
        );
    }

    const DIAGRAM: &str = "requirementDiagram\n\
        requirement test_req {\n\
        id: 1\n\
        text: the test text.\n\
        risk: high\n\
        verifymethod: test\n\
        }\n\
        element test_entity {\n\
        type: simulation\n\
        docref: ./spec.md\n\
        }\n\
        test_entity - satisfies -> test_req";

    #[test]
    fn a_whole_diagram_reads() {
        let diagram = parse(DIAGRAM);
        assert_eq!(diagram.requirements.len(), 1);
        assert_eq!(diagram.requirements[0].name, "test_req");
        assert_eq!(diagram.requirements[0].id.as_deref(), Some("1"));
        assert_eq!(diagram.requirements[0].risk.as_deref(), Some("high"));
        assert_eq!(diagram.elements[0].kind.as_deref(), Some("simulation"));
        assert_eq!(diagram.relationships.len(), 1);
    }

    #[test]
    fn every_requirement_keyword_reads() {
        let source = "requirementDiagram\n\
            functionalRequirement a {\n}\n\
            interfaceRequirement b {\n}\n\
            performanceRequirement c {\n}\n\
            physicalRequirement d {\n}\n\
            designConstraint e {\n}";
        let kinds: Vec<Kind> = parse(source).requirements.iter().map(|r| r.kind).collect();
        assert_eq!(
            kinds,
            [
                Kind::Functional,
                Kind::Interface,
                Kind::Performance,
                Kind::Physical,
                Kind::DesignConstraint,
            ]
        );
    }

    #[test]
    fn a_stereotype_splits_the_keyword_at_its_humps() {
        assert_eq!(Kind::Functional.stereotype(), "«Functional Requirement»");
        assert_eq!(Kind::Requirement.stereotype(), "«Requirement»");
        assert_eq!(Kind::DesignConstraint.stereotype(), "«Design Constraint»");
    }

    #[test]
    fn a_relationship_reads_the_same_written_either_way() {
        let forward = parse("requirementDiagram\na - satisfies -> b");
        let reverse = parse("requirementDiagram\nb <- satisfies - a");
        assert_eq!(forward.relationships, reverse.relationships);
        assert_eq!(forward.relationships[0].source, "a");
        assert_eq!(forward.relationships[0].dest, "b");
    }

    #[test]
    fn a_name_may_contain_a_dash() {
        // The arrow is found first, so `my-req` is not split at its dash.
        let diagram = parse("requirementDiagram\nmy-entity - traces -> my-req");
        assert_eq!(diagram.relationships[0].source, "my-entity");
        assert_eq!(diagram.relationships[0].dest, "my-req");
    }

    #[test]
    fn an_unknown_field_is_ignored_rather_than_fatal() {
        let diagram = parse("requirementDiagram\nrequirement r {\nnonsense: x\nid: 7\n}");
        assert_eq!(diagram.requirements[0].id.as_deref(), Some("7"));
    }

    #[test]
    fn an_unknown_block_keyword_opens_nothing() {
        // Its body has to be read at the top level, not attached to whatever
        // block happened to come before it.
        let diagram = parse("requirementDiagram\nrequirement r {\nid: 1\n}\nmystery m {\nid: 2\n}");
        assert_eq!(diagram.requirements.len(), 1);
        assert_eq!(diagram.requirements[0].id.as_deref(), Some("1"));
    }

    #[test]
    fn a_block_with_no_fields_is_still_a_block() {
        let diagram = parse("requirementDiagram\nrequirement bare {\n}");
        assert_eq!(diagram.requirements.len(), 1);
        assert_eq!(diagram.requirements[0].id, None);
    }

    #[test]
    fn quotes_around_a_value_are_optional() {
        let diagram = parse("requirementDiagram\nrequirement r {\ntext: \"quoted\"\n}");
        assert_eq!(diagram.requirements[0].text.as_deref(), Some("quoted"));
    }

    #[test]
    fn nothing_in_yields_an_empty_diagram() {
        assert_eq!(parse(""), Diagram::default());
        assert_eq!(parse("requirementDiagram"), Diagram::default());
    }
}
