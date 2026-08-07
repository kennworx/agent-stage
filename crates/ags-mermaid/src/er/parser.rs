//! Reading `erDiagram` source.
//!
//! Two kinds of line: an entity with its columns in braces, and a relationship
//! written as two cardinalities either side of a line style. Both are read by
//! free functions over one line, so the reader itself only decides which to try.

use crate::text::normalize_label;

use super::types::{Attribute, Cardinality, Diagram, Entity, Key, Relationship};

const HEADER: &str = "erdiagram";
/// The characters a cardinality is written from.
const MARKS: [char; 4] = ['|', 'o', '{', '}'];

/// The first two words of a line, and whatever is left after them.
fn two_words(line: &str) -> Option<(&str, &str, &str)> {
    let (first, rest) = line.trim().split_once(char::is_whitespace)?;
    let rest = rest.trim_start();
    let (second, tail) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    (!second.is_empty()).then_some((first, second, tail.trim()))
}

/// The text inside the first pair of quotes.
fn quoted(text: &str) -> Option<(&str, usize, usize)> {
    let open = text.find('"')?;
    let rest = text.get(open + 1..)?;
    let close = rest.find('"')?;
    Some((rest.get(..close)?, open, open + close + 2))
}

/// One column: `type name [PK|FK|UK …] ["comment"]`.
pub fn attribute(line: &str) -> Option<Attribute> {
    let (kind, name, rest) = two_words(line)?;
    let (comment, without) = match quoted(rest) {
        Some((body, open, after)) => {
            let head = rest.get(..open).unwrap_or("");
            let tail = rest.get(after..).unwrap_or("");
            (normalize_label(body), format!("{head} {tail}"))
        }
        None => (String::new(), rest.to_string()),
    };
    Some(Attribute {
        kind: kind.to_string(),
        name: name.to_string(),
        keys: without
            .split_whitespace()
            .filter_map(Key::from_word)
            .collect(),
        comment,
    })
}

/// An entity block opening: `ORDER {`.
fn opening(line: &str) -> Option<&str> {
    let name = line.strip_suffix('{')?.trim();
    (!name.is_empty() && !name.contains(char::is_whitespace)).then_some(name)
}

/// The two cardinalities and the line style, from the token between the names.
fn notation(token: &str) -> Option<(Cardinality, Cardinality, bool)> {
    let (at, style, identifying) = match (token.find("--"), token.find("..")) {
        (Some(at), _) => (at, "--", true),
        (None, Some(at)) => (at, "..", false),
        (None, None) => return None,
    };
    let left = token.get(..at)?;
    let right = token.get(at + style.len()..)?;
    Some((
        Cardinality::from_notation(left)?,
        Cardinality::from_notation(right)?,
        identifying,
    ))
}

/// Whether a word is written entirely from the cardinality alphabet.
fn is_notation(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .all(|mark| MARKS.contains(&mark) || mark == '-' || mark == '.')
}

/// A relationship line: `ORDER ||--|{ LINE_ITEM : contains`.
///
/// The label is optional here though Mermaid asks for one: a diagram missing a
/// colon should draw with an unnamed line rather than lose the relationship.
pub fn relationship(line: &str) -> Option<Relationship> {
    let (from, token, rest) = two_words(line)?;
    if !is_notation(token) {
        return None;
    }
    let (from_cardinality, to_cardinality, identifying) = notation(token)?;
    let (to, label) = match rest.split_once(':') {
        Some((name, label)) => (name.trim(), label.trim()),
        None => (rest, ""),
    };
    if to.is_empty() || to.contains(char::is_whitespace) {
        return None;
    }
    Some(Relationship {
        from: from.to_string(),
        to: to.to_string(),
        from_cardinality,
        to_cardinality,
        // The label is written quoted as often as not, and the quotes are not
        // part of the verb.
        label: normalize_label(label.trim_matches(['"', '\''])),
        identifying,
    })
}

/// The reader's state: which entity body is open.
#[derive(Default)]
struct Reader {
    diagram: Diagram,
    open: Option<usize>,
}

impl Reader {
    /// The entity called `id`, declaring it if this is the first mention.
    fn ensure(&mut self, id: &str) -> usize {
        if let Some(at) = self.diagram.index_of(id) {
            return at;
        }
        self.diagram.entities.push(Entity {
            id: id.to_string(),
            label: id.to_string(),
            attributes: Vec::new(),
        });
        self.diagram.entities.len() - 1
    }

    /// One line inside an entity body.
    fn body(&mut self, at: usize, line: &str) {
        if line == "}" {
            self.open = None;
            return;
        }
        let Some(parsed) = attribute(line) else {
            return;
        };
        if let Some(entity) = self.diagram.entities.get_mut(at) {
            entity.attributes.push(parsed);
        }
    }

    /// One line outside any entity body.
    fn statement(&mut self, line: &str) {
        if let Some(name) = opening(line) {
            self.open = Some(self.ensure(name));
            return;
        }
        if let Some(rel) = relationship(line) {
            self.ensure(&rel.from);
            self.ensure(&rel.to);
            self.diagram.relationships.push(rel);
        }
    }

    fn line(&mut self, text: &str) {
        match self.open {
            Some(at) => self.body(at, text),
            None => self.statement(text),
        }
    }
}

/// Read an ER diagram.
///
/// A line nobody recognises is dropped rather than rejected: a diagram with one
/// bad line should still draw the rest of itself.
pub fn parse(source: &str) -> Diagram {
    let mut reader = Reader::default();
    let mut seen_header = false;
    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if !seen_header {
            seen_header = true;
            if line.to_ascii_lowercase().starts_with(HEADER) {
                continue;
            }
        }
        reader.line(line);
    }
    reader.diagram
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_words_splits_a_line_and_keeps_the_remainder_whole() {
        assert_eq!(
            two_words("int id PK \"the key\""),
            Some(("int", "id", "PK \"the key\""))
        );
        assert_eq!(two_words("int id"), Some(("int", "id", "")));
        assert_eq!(two_words("  int   id  "), Some(("int", "id", "")));
        assert_eq!(two_words("alone"), None);
        assert_eq!(two_words(""), None);
    }

    #[test]
    fn a_quoted_run_is_found_with_the_span_it_occupies() {
        assert_eq!(quoted(r#"PK "a note" FK"#), Some(("a note", 3, 11)));
        assert_eq!(quoted("PK FK"), None);
        assert_eq!(quoted("PK \"unclosed"), None);
    }

    #[test]
    fn an_attribute_is_read_as_a_type_and_a_name() {
        let parsed = attribute("int id").unwrap();
        assert_eq!(parsed.kind, "int");
        assert_eq!(parsed.name, "id");
        assert!(parsed.keys.is_empty());
        assert_eq!(parsed.comment, "");
    }

    #[test]
    fn an_attribute_carries_its_keys_and_its_note() {
        let parsed = attribute(r#"int customer_id FK "who placed it""#).unwrap();
        assert_eq!(parsed.keys, [Key::Foreign]);
        assert_eq!(parsed.comment, "who placed it");
        // Two keys on one column.
        let both = attribute("int id PK FK").unwrap();
        assert_eq!(both.keys, [Key::Primary, Key::Foreign]);
    }

    #[test]
    fn a_note_is_not_mistaken_for_a_key() {
        let parsed = attribute(r#"string state "PK is elsewhere""#).unwrap();
        assert!(parsed.keys.is_empty(), "{:?}", parsed.keys);
        assert_eq!(parsed.comment, "PK is elsewhere");
    }

    #[test]
    fn a_line_with_one_word_is_not_an_attribute() {
        assert_eq!(attribute("id"), None);
        assert_eq!(attribute("   "), None);
    }

    #[test]
    fn an_entity_block_opens_on_a_name_and_a_brace() {
        assert_eq!(opening("ORDER {"), Some("ORDER"));
        assert_eq!(opening("ORDER{"), Some("ORDER"));
        assert_eq!(opening("TWO NAMES {"), None);
        assert_eq!(opening("{"), None);
        assert_eq!(opening("ORDER"), None);
    }

    #[test]
    fn a_notation_reads_both_ends_and_the_line_between_them() {
        assert_eq!(
            notation("||--o{"),
            Some((Cardinality::One, Cardinality::ZeroMany, true))
        );
        assert_eq!(
            notation("|o..|{"),
            Some((Cardinality::ZeroOne, Cardinality::Many, false))
        );
        // A style nobody writes, and ends that say nothing.
        assert_eq!(notation("||~~o{"), None);
        assert_eq!(notation("xx--o{"), None);
        assert_eq!(notation("||--xx"), None);
    }

    #[test]
    fn only_a_word_from_the_cardinality_alphabet_is_a_notation() {
        assert!(is_notation("||--o{"));
        assert!(is_notation("|o..|{"));
        assert!(!is_notation("places"));
        assert!(!is_notation(""));
    }

    #[test]
    fn a_relationship_is_read_with_its_verb() {
        let rel = relationship("CUSTOMER ||--o{ ORDER : places").unwrap();
        assert_eq!(rel.from, "CUSTOMER");
        assert_eq!(rel.to, "ORDER");
        assert_eq!(rel.from_cardinality, Cardinality::One);
        assert_eq!(rel.to_cardinality, Cardinality::ZeroMany);
        assert_eq!(rel.label, "places");
        assert!(rel.identifying);
    }

    #[test]
    fn a_dotted_relationship_is_the_non_identifying_one() {
        let rel = relationship("USER ||..o{ LOG_ENTRY : generates").unwrap();
        assert!(!rel.identifying);
        assert_eq!(rel.label, "generates");
    }

    #[test]
    fn a_relationship_without_a_verb_is_still_a_relationship() {
        let rel = relationship("A ||--|| B").unwrap();
        assert_eq!(rel.to, "B");
        assert_eq!(rel.label, "");
    }

    #[test]
    fn a_quoted_verb_loses_its_quotes() {
        let rel = relationship(r#"A ||--|| B : "belongs to""#).unwrap();
        assert_eq!(rel.label, "belongs to");
    }

    #[test]
    fn a_line_that_is_not_a_relationship_is_not_read_as_one() {
        assert_eq!(relationship("CUSTOMER places ORDER : x"), None);
        assert_eq!(relationship("CUSTOMER ||--o{ TWO NAMES : x"), None);
        assert_eq!(relationship("CUSTOMER ||--o{"), None);
        assert_eq!(relationship("alone"), None);
    }

    #[test]
    fn an_entity_block_becomes_columns_in_the_order_written() {
        let diagram = parse(
            "erDiagram\n  CUSTOMER {\n    int id PK\n    string name\n    string email UK\n  }",
        );
        let customer = diagram.entities.first().unwrap();
        assert_eq!(customer.id, "CUSTOMER");
        assert_eq!(
            customer
                .attributes
                .iter()
                .map(|a| a.name.clone())
                .collect::<Vec<String>>(),
            ["id", "name", "email"]
        );
        assert_eq!(customer.attributes.first().unwrap().keys, [Key::Primary]);
    }

    #[test]
    fn a_relationship_declares_the_entities_it_names() {
        let diagram = parse("erDiagram\n  CUSTOMER ||--o{ ORDER : places");
        assert_eq!(
            diagram
                .entities
                .iter()
                .map(|e| e.id.clone())
                .collect::<Vec<String>>(),
            ["CUSTOMER", "ORDER"]
        );
        assert_eq!(diagram.relationships.len(), 1);
    }

    #[test]
    fn an_entity_named_twice_is_one_entity() {
        let diagram =
            parse("erDiagram\n  CUSTOMER ||--o{ ORDER : places\n  CUSTOMER {\n    int id PK\n  }");
        assert_eq!(diagram.entities.len(), 2);
        assert_eq!(diagram.entities.first().unwrap().attributes.len(), 1);
    }

    #[test]
    fn comments_blank_lines_and_nonsense_are_all_stepped_over() {
        let diagram = parse(
            "erDiagram\n\n  %% a note\n  A ||--|| B : x\n  ??? nothing ???\n  C ||--|| D : y",
        );
        assert_eq!(diagram.relationships.len(), 2);
        assert_eq!(diagram.entities.len(), 4);
    }

    #[test]
    fn a_source_with_no_header_still_reads() {
        let diagram = parse("A ||--|| B : x");
        assert_eq!(diagram.relationships.len(), 1);
    }

    #[test]
    fn a_body_that_is_never_closed_ends_with_the_source() {
        let diagram = parse("erDiagram\n  A {\n    int id PK\n    string name");
        assert_eq!(diagram.entities.len(), 1);
        assert_eq!(diagram.entities.first().unwrap().attributes.len(), 2);
    }
}
