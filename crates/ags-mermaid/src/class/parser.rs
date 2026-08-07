//! Reading `classDiagram` source.
//!
//! Every interesting decision here is a free function over one line, so it can
//! be tested by handing it that line. The reader itself only decides which of
//! them to try, and in what order.
//!
//! `namespace` blocks are recognised and stepped over rather than modelled. The
//! classes inside them are declared as normal; nothing is drawn round them,
//! which is what the renderer this replaces did too. When frames round a group
//! of classes become worth drawing, the membership is one line of parsing away
//! — recording it now would be a field nobody reads.

use crate::text::normalize_label;

use super::types::{Class, Diagram, End, Member, Relation, Relationship, Visibility, ARROWS};

const HEADER: &str = "classdiagram";

/// A `class` line, whatever form it was written in.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Declaration {
    id: String,
    label: String,
    /// A body follows on the lines below.
    opens: bool,
    /// A `<<stereotype>>` written inside braces on this same line.
    annotation: String,
}

/// The text after a leading keyword, when the line starts with it.
fn after_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    // The keyword has to be a word, not a prefix: `classy` is not `class`.
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some(rest.trim())
}

/// The stereotype a `<<…>>` line names.
fn annotation(line: &str) -> Option<&str> {
    let inner = line.strip_prefix("<<")?.strip_suffix(">>")?.trim();
    (!inner.is_empty()).then_some(inner)
}

/// A name and the label it is drawn with, splitting off a `~T~` generic.
///
/// `Box~Item~` is drawn `Box<Item>`: the tildes are Mermaid's way of writing
/// angle brackets in a syntax where `<` already means something.
fn generic(name: &str) -> (String, String) {
    let Some(body) = name.strip_suffix('~') else {
        return (name.to_string(), name.to_string());
    };
    match body.split_once('~') {
        Some((id, param)) if !id.is_empty() && !param.is_empty() => {
            (id.to_string(), format!("{id}<{param}>"))
        }
        _ => (name.to_string(), name.to_string()),
    }
}

/// What a `class …` line declares.
fn declaration(line: &str) -> Option<Declaration> {
    let rest = after_keyword(line, "class")?;
    let mut head = rest;
    let mut opens = false;
    let mut annot = String::new();
    if let Some(open) = rest.find('{') {
        let inside = rest.get(open + 1..)?.trim();
        head = rest.get(..open)?.trim();
        match inside.strip_suffix('}') {
            // `class A {}` — a body that says nothing.
            Some(body) if body.trim().is_empty() => {}
            // `class A { <<interface>> }` — the whole body on one line. Only a
            // stereotype fits there; anything else is a line this does not read,
            // and half-reading it would silently drop whatever was inside.
            Some(body) => annot = annotation(body.trim())?.to_string(),
            // `class A {` — the body is on the lines below.
            None if inside.is_empty() => opens = true,
            None => return None,
        }
    }
    let (id, label) = generic(head);
    if id.is_empty() || id.contains(char::is_whitespace) {
        return None;
    }
    Some(Declaration {
        id,
        label,
        opens,
        annotation: annot,
    })
}

/// A trailing `$` or `*`: static, and abstract. Returns the text without it.
fn classifier(text: &str) -> (&str, bool, bool) {
    if let Some(head) = text.strip_suffix('$') {
        return (head, true, false);
    }
    if let Some(head) = text.strip_suffix('*') {
        return (head, false, true);
    }
    (text, false, false)
}

/// The visibility a line opens with, and what is left after it.
fn visibility_of(text: &str) -> (Visibility, &str) {
    let Some(first) = text.chars().next() else {
        return (Visibility::Unstated, text);
    };
    match Visibility::from_mark(first) {
        Some(visibility) => (
            visibility,
            text.get(first.len_utf8()..).unwrap_or("").trim(),
        ),
        None => (Visibility::Unstated, text),
    }
}

/// A method: a name, a parameter list, and whatever follows as the return type.
fn method(rest: &str, open: usize) -> Option<Member> {
    let close = rest.get(open..)?.find(')')? + open;
    let (name, is_static, is_abstract) = classifier(rest.get(..open)?.trim());
    let params = rest.get(open + 1..close)?.trim().to_string();
    let mut tail = rest.get(close + 1..)?.trim();
    // `+read()$ String` — the classifier can sit between the parentheses and the
    // return type as well as at the end of the name.
    let mut is_static = is_static;
    let mut is_abstract = is_abstract;
    if let Some(after) = tail.strip_prefix('$') {
        is_static = true;
        tail = after.trim();
    } else if let Some(after) = tail.strip_prefix('*') {
        is_abstract = true;
        tail = after.trim();
    }
    Some(Member {
        visibility: Visibility::Unstated,
        name: name.to_string(),
        kind: tail.to_string(),
        params: Some(params),
        is_static,
        is_abstract,
    })
}

/// A field, written either `Type name` or just `name`.
fn field(rest: &str) -> Member {
    let (text, is_static, is_abstract) = classifier(rest);
    let (kind, name) = match text.split_once(char::is_whitespace) {
        Some((kind, name)) => (kind.trim(), name.trim()),
        None => ("", text),
    };
    Member {
        visibility: Visibility::Unstated,
        name: name.to_string(),
        kind: kind.to_string(),
        params: None,
        is_static,
        is_abstract,
    }
}

/// One line inside a class body.
pub fn member(line: &str) -> Option<Member> {
    let text = line.trim().trim_end_matches(';').trim();
    if text.is_empty() {
        return None;
    }
    let (visibility, rest) = visibility_of(text);
    if rest.is_empty() {
        return None;
    }
    let mut parsed = match rest.find('(') {
        Some(open) => method(rest, open).unwrap_or_else(|| field(rest)),
        None => field(rest),
    };
    parsed.visibility = visibility;
    Some(parsed)
}

/// Where an arrow sits in a line, and what it means.
///
/// Whitespace on both sides is required, which is what keeps `A-->B` from being
/// read and, more usefully, keeps a class named `x--y` from being torn in half.
fn find_arrow(line: &str) -> Option<(usize, usize, Relation, End)> {
    for (at, _) in line.char_indices() {
        let preceded_by_space = line
            .get(..at)
            .and_then(|head| head.chars().next_back())
            .is_some_and(char::is_whitespace);
        if !preceded_by_space {
            continue;
        }
        let tail = line.get(at..)?;
        for (arrow, kind, end) in ARROWS {
            let Some(after) = tail.strip_prefix(arrow) else {
                continue;
            };
            if after.starts_with(char::is_whitespace) {
                return Some((at, arrow.len(), kind, end));
            }
        }
    }
    None
}

/// A quoted multiplicity at the end of the text before an arrow.
fn tail_cardinality(text: &str) -> Option<(String, String)> {
    let Some(body) = text.strip_suffix('"') else {
        return Some((text.to_string(), String::new()));
    };
    let open = body.rfind('"')?;
    let card = body.get(open + 1..)?.to_string();
    Some((body.get(..open)?.trim().to_string(), card))
}

/// A quoted multiplicity at the start of the text after an arrow.
fn head_cardinality(text: &str) -> Option<(String, String)> {
    let Some(body) = text.strip_prefix('"') else {
        return Some((text.to_string(), String::new()));
    };
    let close = body.find('"')?;
    let card = body.get(..close)?.to_string();
    Some((body.get(close + 1..)?.trim().to_string(), card))
}

/// A name written on its own, or nothing.
fn lone_name(text: &str) -> Option<String> {
    let name = text.trim();
    (!name.is_empty() && !name.contains(char::is_whitespace)).then(|| name.to_string())
}

/// A relationship line: `FROM ["card"] ARROW ["card"] TO [: label]`.
pub fn relationship(line: &str) -> Option<Relationship> {
    let (at, len, kind, marker_at) = find_arrow(line)?;
    let (from_text, from_cardinality) = tail_cardinality(line.get(..at)?.trim())?;
    let (after, to_cardinality) = head_cardinality(line.get(at + len..)?.trim())?;
    let (to_text, label) = match after.split_once(':') {
        Some((name, label)) => (name.to_string(), label.trim().to_string()),
        None => (after, String::new()),
    };
    Some(Relationship {
        from: lone_name(&from_text)?,
        to: lone_name(&to_text)?,
        kind,
        marker_at,
        label: normalize_label(&label),
        from_cardinality: normalize_label(&from_cardinality),
        to_cardinality: normalize_label(&to_cardinality),
    })
}

/// An `A : +String name` line, which puts a member on a class from outside it.
fn inline_member(line: &str) -> Option<(String, Member)> {
    let (head, rest) = line.split_once(':')?;
    Some((lone_name(head)?, member(rest)?))
}

/// Put a member in the compartment it belongs to. The parentheses are the only
/// thing that says which one that is.
fn hold(class: &mut Class, parsed: Member) {
    if parsed.is_method() {
        class.methods.push(parsed);
    } else {
        class.attributes.push(parsed);
    }
}

/// The reader's state: which class body is open, and which namespace.
#[derive(Default)]
struct Reader {
    diagram: Diagram,
    /// The class whose body the following lines belong to.
    open: Option<usize>,
    in_namespace: bool,
}

impl Reader {
    /// The class called `id`, declaring it if this is the first mention.
    ///
    /// A relationship may name a class before — or instead of — any `class`
    /// line for it, and a diagram that is nothing but relationships is a normal
    /// way to write one.
    fn ensure(&mut self, id: &str) -> usize {
        if let Some(at) = self.diagram.index_of(id) {
            return at;
        }
        self.diagram.classes.push(Class {
            id: id.to_string(),
            label: id.to_string(),
            ..Class::default()
        });
        self.diagram.classes.len() - 1
    }

    fn declare(&mut self, decl: &Declaration) {
        let at = self.ensure(&decl.id);
        if let Some(class) = self.diagram.classes.get_mut(at) {
            class.label.clone_from(&decl.label);
            if !decl.annotation.is_empty() {
                class.annotation.clone_from(&decl.annotation);
            }
        }
        self.open = decl.opens.then_some(at);
    }

    fn attach(&mut self, at: usize, parsed: Member) {
        if let Some(class) = self.diagram.classes.get_mut(at) {
            hold(class, parsed);
        }
    }

    /// One line inside a class body.
    fn body(&mut self, at: usize, line: &str) {
        if line == "}" {
            self.open = None;
            return;
        }
        if let Some(name) = annotation(line) {
            if let Some(class) = self.diagram.classes.get_mut(at) {
                class.annotation = name.to_string();
            }
            return;
        }
        if let Some(parsed) = member(line) {
            self.attach(at, parsed);
        }
    }

    /// One line outside any class body.
    fn statement(&mut self, line: &str) {
        if let Some(rest) = after_keyword(line, "namespace") {
            if rest.ends_with('{') {
                self.in_namespace = true;
                return;
            }
        }
        if line == "}" {
            self.in_namespace = false;
            return;
        }
        if let Some(decl) = declaration(line) {
            self.declare(&decl);
            return;
        }
        if let Some(rel) = relationship(line) {
            self.ensure(&rel.from);
            self.ensure(&rel.to);
            self.diagram.relationships.push(rel);
            return;
        }
        if let Some((id, parsed)) = inline_member(line) {
            let at = self.ensure(&id);
            self.attach(at, parsed);
        }
    }

    fn line(&mut self, text: &str) {
        match self.open {
            Some(at) => self.body(at, text),
            None => self.statement(text),
        }
    }
}

/// Read a class diagram.
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
    fn a_keyword_has_to_be_a_whole_word() {
        assert_eq!(after_keyword("class Animal", "class"), Some("Animal"));
        assert_eq!(after_keyword("class   Animal  ", "class"), Some("Animal"));
        assert_eq!(after_keyword("classy Animal", "class"), None);
        assert_eq!(after_keyword("Animal", "class"), None);
        assert_eq!(after_keyword("class", "class"), None);
    }

    #[test]
    fn a_stereotype_is_read_from_its_brackets() {
        assert_eq!(annotation("<<interface>>"), Some("interface"));
        assert_eq!(annotation("<< abstract >>"), Some("abstract"));
        assert_eq!(annotation("<<>>"), None);
        assert_eq!(annotation("<interface>"), None);
        assert_eq!(annotation("interface"), None);
    }

    #[test]
    fn a_generic_is_drawn_with_angle_brackets() {
        assert_eq!(
            generic("Box~Item~"),
            ("Box".to_string(), "Box<Item>".to_string())
        );
        assert_eq!(generic("Box"), ("Box".to_string(), "Box".to_string()));
        // Nothing between the tildes, and nothing before them, are both left be.
        assert_eq!(generic("Box~~"), ("Box~~".to_string(), "Box~~".to_string()));
        assert_eq!(
            generic("~Item~"),
            ("~Item~".to_string(), "~Item~".to_string())
        );
    }

    #[test]
    fn a_class_line_is_read_in_every_form_it_is_written() {
        assert_eq!(
            declaration("class Animal"),
            Some(Declaration {
                id: "Animal".into(),
                label: "Animal".into(),
                opens: false,
                annotation: String::new(),
            })
        );
        assert_eq!(
            declaration("class Animal {"),
            Some(Declaration {
                id: "Animal".into(),
                label: "Animal".into(),
                opens: true,
                annotation: String::new(),
            })
        );
        assert_eq!(
            declaration("class Shape { <<abstract>> }"),
            Some(Declaration {
                id: "Shape".into(),
                label: "Shape".into(),
                opens: false,
                annotation: "abstract".into(),
            })
        );
        assert_eq!(
            declaration("class Box~Item~ {"),
            Some(Declaration {
                id: "Box".into(),
                label: "Box<Item>".into(),
                opens: true,
                annotation: String::new(),
            })
        );
        assert_eq!(declaration("class Box {}"), {
            Some(Declaration {
                id: "Box".into(),
                label: "Box".into(),
                opens: false,
                annotation: String::new(),
            })
        });
    }

    #[test]
    fn a_class_line_that_says_something_else_is_not_read_as_one() {
        // A body with a member on the same line is not a form this reads.
        assert_eq!(declaration("class A { +int x }"), None);
        // Two names is not one name.
        assert_eq!(declaration("class A B"), None);
        assert_eq!(declaration("class { }"), None);
        assert_eq!(declaration("Animal <|-- Dog"), None);
    }

    #[test]
    fn a_trailing_marker_says_static_or_abstract() {
        assert_eq!(classifier("count$"), ("count", true, false));
        assert_eq!(classifier("area*"), ("area", false, true));
        assert_eq!(classifier("name"), ("name", false, false));
    }

    #[test]
    fn a_leading_character_says_who_may_see_a_member() {
        assert_eq!(visibility_of("+name"), (Visibility::Public, "name"));
        assert_eq!(visibility_of("- name"), (Visibility::Private, "name"));
        assert_eq!(visibility_of("#x"), (Visibility::Protected, "x"));
        assert_eq!(visibility_of("~x"), (Visibility::Package, "x"));
        assert_eq!(visibility_of("name"), (Visibility::Unstated, "name"));
        assert_eq!(visibility_of(""), (Visibility::Unstated, ""));
    }

    #[test]
    fn a_field_is_read_as_a_type_and_a_name() {
        let parsed = member("+String name").unwrap();
        assert_eq!(parsed.visibility, Visibility::Public);
        assert_eq!(parsed.name, "name");
        assert_eq!(parsed.kind, "String");
        assert!(!parsed.is_method());
    }

    #[test]
    fn a_field_with_no_type_is_just_a_name() {
        let parsed = member("ACTIVE").unwrap();
        assert_eq!(parsed.name, "ACTIVE");
        assert_eq!(parsed.kind, "");
        assert_eq!(parsed.visibility, Visibility::Unstated);
    }

    #[test]
    fn a_generic_field_type_is_kept_whole() {
        let parsed = member("-List~Observer~ observers").unwrap();
        assert_eq!(parsed.kind, "List~Observer~");
        assert_eq!(parsed.name, "observers");
    }

    #[test]
    fn a_method_is_read_as_a_name_parameters_and_a_return_type() {
        let parsed = member("+setData(key, val) void").unwrap();
        assert_eq!(parsed.name, "setData");
        assert_eq!(parsed.params.as_deref(), Some("key, val"));
        assert_eq!(parsed.kind, "void");
        assert!(parsed.is_method());
    }

    #[test]
    fn a_method_with_no_parameters_is_still_a_method() {
        let parsed = member("+eat() void").unwrap();
        assert_eq!(parsed.params.as_deref(), Some(""));
        assert!(parsed.is_method());
    }

    #[test]
    fn a_classifier_is_read_wherever_it_is_written() {
        // On the end of a field, which is where UML puts it.
        let counted = member("+int count$").unwrap();
        assert!(counted.is_static);
        assert_eq!(counted.name, "count");
        assert_eq!(counted.kind, "int");
        // On the end of a method name.
        let named = member("+of$() Thing").unwrap();
        assert!(named.is_static);
        // And between the parentheses and the return type.
        let after = member("+area()* double").unwrap();
        assert!(after.is_abstract);
        assert_eq!(after.kind, "double");
    }

    #[test]
    fn a_trailing_semicolon_and_an_empty_line_are_not_members() {
        assert_eq!(member("+int age;").unwrap().name, "age");
        assert_eq!(member("   "), None);
        assert_eq!(member("+"), None);
    }

    #[test]
    fn an_unclosed_parenthesis_reads_as_a_field_rather_than_failing() {
        let parsed = member("+broken(void").unwrap();
        assert!(!parsed.is_method());
        assert_eq!(parsed.kind, "");
        assert_eq!(parsed.name, "broken(void");
    }

    #[test]
    fn an_arrow_is_found_only_when_it_stands_alone() {
        let (at, len, kind, end) = find_arrow("Animal <|-- Dog").unwrap();
        assert_eq!(at, 7);
        assert_eq!(len, 4);
        assert_eq!(kind, Relation::Inheritance);
        assert_eq!(end, End::From);
        // Without space around it there is no arrow to find.
        assert_eq!(find_arrow("Animal<|--Dog"), None);
        assert_eq!(find_arrow("Animal Dog"), None);
    }

    #[test]
    fn a_long_arrow_is_not_read_as_the_short_one_inside_it() {
        let (_, len, kind, end) = find_arrow("Bird ..|> Flyable").unwrap();
        assert_eq!(len, 4);
        assert_eq!(kind, Relation::Realization);
        assert_eq!(end, End::To);
        let (_, len, kind, _) = find_arrow("A --|> B").unwrap();
        assert_eq!(len, 4);
        assert_eq!(kind, Relation::Inheritance);
        // And the bare line is still an arrow in its own right.
        let (_, len, kind, _) = find_arrow("A -- B").unwrap();
        assert_eq!(len, 2);
        assert_eq!(kind, Relation::Association);
    }

    #[test]
    fn every_arrow_in_the_table_is_read_back_as_what_it_means() {
        for (arrow, kind, end) in ARROWS {
            let line = format!("A {arrow} B");
            let rel = relationship(&line).unwrap_or_else(|| panic!("{arrow}"));
            assert_eq!(rel.kind, kind, "{arrow}");
            assert_eq!(rel.marker_at, end, "{arrow}");
            assert_eq!(rel.from, "A");
            assert_eq!(rel.to, "B");
        }
    }

    #[test]
    fn a_relationship_carries_its_label() {
        let rel = relationship("Teacher --> Course : teaches").unwrap();
        assert_eq!(rel.from, "Teacher");
        assert_eq!(rel.to, "Course");
        assert_eq!(rel.label, "teaches");
        // A label with spaces in it stays whole.
        let rel = relationship("Student --> Course : enrolled in").unwrap();
        assert_eq!(rel.label, "enrolled in");
    }

    #[test]
    fn a_relationship_carries_the_multiplicity_at_each_end() {
        let rel = relationship(r#"Order "1" --> "*" Item : holds"#).unwrap();
        assert_eq!(rel.from, "Order");
        assert_eq!(rel.to, "Item");
        assert_eq!(rel.from_cardinality, "1");
        assert_eq!(rel.to_cardinality, "*");
        assert_eq!(rel.label, "holds");
    }

    #[test]
    fn a_line_with_two_names_on_one_side_is_not_a_relationship() {
        assert_eq!(relationship("A B --> C"), None);
        assert_eq!(relationship("A --> B C"), None);
        assert_eq!(relationship("--> B"), None);
    }

    #[test]
    fn a_multiplicity_is_read_from_either_end_alone() {
        let from = relationship(r#"A "1" --> B"#).unwrap();
        assert_eq!(from.from_cardinality, "1");
        assert_eq!(from.to_cardinality, "");
        let to = relationship(r#"A --> "0..*" B"#).unwrap();
        assert_eq!(to.from_cardinality, "");
        assert_eq!(to.to_cardinality, "0..*");
    }

    #[test]
    fn an_inline_member_names_the_class_it_belongs_to() {
        let (id, parsed) = inline_member("Animal : +String name").unwrap();
        assert_eq!(id, "Animal");
        assert_eq!(parsed.name, "name");
        assert_eq!(inline_member("Animal <|-- Dog"), None);
        assert_eq!(inline_member(": +String name"), None);
    }

    #[test]
    fn a_class_body_becomes_fields_and_methods_in_the_order_written() {
        let diagram = parse(
            "classDiagram\n  class Animal {\n    +String name\n    +int age\n    +eat() void\n    +sleep() void\n  }",
        );
        let animal = diagram.classes.first().unwrap();
        assert_eq!(animal.id, "Animal");
        assert_eq!(
            animal
                .attributes
                .iter()
                .map(|m| m.name.clone())
                .collect::<Vec<String>>(),
            ["name", "age"]
        );
        assert_eq!(
            animal
                .methods
                .iter()
                .map(|m| m.name.clone())
                .collect::<Vec<String>>(),
            ["eat", "sleep"]
        );
    }

    #[test]
    fn a_stereotype_written_on_the_class_line_lands_on_the_class() {
        let diagram = parse("classDiagram\n  class Shape { <<abstract>> }\n  Shape <|-- Circle");
        let shape = diagram.classes.first().unwrap();
        assert_eq!(shape.id, "Shape");
        assert_eq!(shape.annotation, "abstract");
        // And a later mention does not take it away again.
        let named = parse("classDiagram\n  class Shape { <<abstract>> }\n  class Shape");
        assert_eq!(named.classes.first().unwrap().annotation, "abstract");
    }

    #[test]
    fn a_stereotype_inside_a_body_lands_on_the_class() {
        let diagram = parse(
            "classDiagram\n  class Serializable {\n    <<interface>>\n    +serialize() String\n  }",
        );
        let first = diagram.classes.first().unwrap();
        assert_eq!(first.annotation, "interface");
        assert_eq!(first.methods.len(), 1);
    }

    #[test]
    fn a_relationship_declares_the_classes_it_names() {
        let diagram = parse("classDiagram\n  A <|-- B : inheritance\n  C *-- D");
        assert_eq!(
            diagram
                .classes
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<String>>(),
            ["A", "B", "C", "D"]
        );
        assert_eq!(diagram.relationships.len(), 2);
        assert_eq!(diagram.relationships.first().unwrap().label, "inheritance");
    }

    #[test]
    fn a_class_named_twice_is_one_class() {
        let diagram = parse(
            "classDiagram\n  Animal <|-- Dog\n  class Animal {\n    +String name\n  }\n  class Dog\n",
        );
        assert_eq!(diagram.classes.len(), 2);
        assert_eq!(diagram.classes.first().unwrap().attributes.len(), 1);
    }

    #[test]
    fn a_namespace_block_is_stepped_over_and_its_classes_kept() {
        let diagram = parse(
            "classDiagram\n  namespace Shapes {\n    class Circle\n    class Square\n  }\n  Circle --> Square",
        );
        assert_eq!(
            diagram
                .classes
                .iter()
                .map(|c| c.id.clone())
                .collect::<Vec<String>>(),
            ["Circle", "Square"]
        );
        assert_eq!(diagram.relationships.len(), 1);
    }

    #[test]
    fn comments_and_blank_lines_are_skipped() {
        let diagram = parse("classDiagram\n\n  %% a note to a reader\n  class A\n");
        assert_eq!(diagram.classes.len(), 1);
    }

    #[test]
    fn a_source_with_no_header_still_reads() {
        // The header is only skipped when it is there, so a fragment handed in
        // without one does not lose its first line.
        let diagram = parse("class A\nclass B");
        assert_eq!(diagram.classes.len(), 2);
    }

    #[test]
    fn a_line_nobody_recognises_is_dropped_rather_than_stopping_the_read() {
        let diagram = parse("classDiagram\n  class A\n  ??? nonsense ???\n  class B");
        assert_eq!(diagram.classes.len(), 2);
    }

    #[test]
    fn an_inline_member_line_lands_on_its_class() {
        let diagram = parse("classDiagram\n  Animal : +String name\n  Animal : +eat() void");
        let animal = diagram.classes.first().unwrap();
        assert_eq!(animal.attributes.len(), 1);
        assert_eq!(animal.methods.len(), 1);
    }
}
