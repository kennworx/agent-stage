//! The parsed shape of a class diagram: boxes with compartments, and the UML
//! relationships between them.
//!
//! A class box is the first thing in this renderer that is not one label in one
//! outline. It has three compartments — the name, the fields, the methods — and
//! each member line is written from parts that are drawn differently. That is
//! what most of this file is: naming those parts so the renderer does not have
//! to re-read the source text to colour them.

/// Who may see a member. Written as one character in front of its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    /// Nothing was written, which is not the same as public.
    #[default]
    Unstated,
    Public,
    Private,
    Protected,
    Package,
}

impl Visibility {
    /// The character this is written with. Empty when nothing was written.
    pub const fn mark(self) -> &'static str {
        match self {
            Self::Unstated => "",
            Self::Public => "+",
            Self::Private => "-",
            Self::Protected => "#",
            Self::Package => "~",
        }
    }

    /// The visibility a leading character names.
    pub const fn from_mark(mark: char) -> Option<Self> {
        match mark {
            '+' => Some(Self::Public),
            '-' => Some(Self::Private),
            '#' => Some(Self::Protected),
            '~' => Some(Self::Package),
            _ => None,
        }
    }
}

/// One line inside a class box: a field or a method.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Member {
    pub visibility: Visibility,
    pub name: String,
    /// The type of a field, or the return type of a method. Empty when unwritten.
    pub kind: String,
    /// The parameter list. `Some` — even when empty — is what makes this a
    /// method rather than a field, because `run()` and `run` are different
    /// things written the same way but for the parentheses.
    pub params: Option<String>,
    /// A trailing `$`. Drawn underlined, as UML asks.
    pub is_static: bool,
    /// A trailing `*`. Drawn italic.
    pub is_abstract: bool,
}

impl Member {
    pub const fn is_method(&self) -> bool {
        self.params.is_some()
    }

    /// The name as it appears in the box, with parentheses when it is a method.
    pub fn written(&self) -> String {
        match &self.params {
            Some(params) => format!("{}({params})", self.name),
            None => self.name.clone(),
        }
    }

    /// The whole line as one string. Only sizing needs this — the renderer draws
    /// the parts separately, because they are not the same colour.
    pub fn line(&self) -> String {
        let mark = self.visibility.mark();
        let lead = if mark.is_empty() {
            String::new()
        } else {
            format!("{mark} ")
        };
        let tail = if self.kind.is_empty() {
            String::new()
        } else {
            format!(": {}", self.kind)
        };
        format!("{lead}{}{tail}", self.written())
    }
}

/// What one class has to do with another, in UML's vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Relation {
    /// `<|--` — B is an A.
    Inheritance,
    /// `*--` — A owns B, and B dies with it.
    Composition,
    /// `o--` — A holds B, which outlives it.
    Aggregation,
    /// `-->` — A knows B.
    Association,
    /// `..>` — A uses B in passing.
    Dependency,
    /// `..|>` — B promises what A declares.
    Realization,
}

impl Relation {
    /// The name this is known by, which becomes its class.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Inheritance => "inheritance",
            Self::Composition => "composition",
            Self::Aggregation => "aggregation",
            Self::Association => "association",
            Self::Dependency => "dependency",
            Self::Realization => "realization",
        }
    }

    /// Whether the line is broken. UML draws the two "at a distance"
    /// relationships dashed and the three structural ones solid.
    pub const fn dashed(self) -> bool {
        matches!(self, Self::Dependency | Self::Realization)
    }

    /// The marker drawn on whichever end carries one.
    ///
    /// Inheritance and realization share a hollow triangle, and association and
    /// dependency share an open arrow; the line style is what tells each pair
    /// apart.
    pub const fn marker(self) -> &'static str {
        match self {
            Self::Inheritance | Self::Realization => INHERIT_MARKER,
            Self::Composition => COMPOSITION_MARKER,
            Self::Aggregation => AGGREGATION_MARKER,
            Self::Association | Self::Dependency => ARROW_MARKER,
        }
    }
}

pub const INHERIT_MARKER: &str = "cls-inherit";
pub const COMPOSITION_MARKER: &str = "cls-composition";
pub const AGGREGATION_MARKER: &str = "cls-aggregation";
pub const ARROW_MARKER: &str = "cls-arrow";

/// Which end of a relationship carries the marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    From,
    To,
}

/// Every arrow, and what it means.
///
/// The same relationship can be written either way round — `A <|-- B` and
/// `B --|> A` say the same thing — and which way it was written decides which
/// end the triangle sits on. Longest first, so `--|>` is never read as `--`.
pub const ARROWS: [(&str, Relation, End); 13] = [
    ("<|--", Relation::Inheritance, End::From),
    ("--|>", Relation::Inheritance, End::To),
    ("<|..", Relation::Realization, End::From),
    ("..|>", Relation::Realization, End::To),
    ("*--", Relation::Composition, End::From),
    ("--*", Relation::Composition, End::To),
    ("o--", Relation::Aggregation, End::From),
    ("--o", Relation::Aggregation, End::To),
    ("-->", Relation::Association, End::To),
    ("<--", Relation::Association, End::From),
    ("..>", Relation::Dependency, End::To),
    ("<..", Relation::Dependency, End::From),
    ("--", Relation::Association, End::To),
];

/// One box.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Class {
    pub id: String,
    /// What is drawn in the header, which differs from the id for a generic.
    pub label: String,
    /// A `<<stereotype>>` above the name. Empty when there is none.
    pub annotation: String,
    pub attributes: Vec<Member>,
    pub methods: Vec<Member>,
}

/// One line between two boxes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub kind: Relation,
    pub marker_at: End,
    pub label: String,
    /// The multiplicity written by each end, e.g. `1` or `0..*`.
    pub from_cardinality: String,
    pub to_cardinality: String,
}

/// A parsed class diagram.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diagram {
    /// Classes in the order they were first named.
    pub classes: Vec<Class>,
    pub relationships: Vec<Relationship>,
}

impl Diagram {
    /// Where `id` sits in `classes`.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.classes.iter().position(|class| class.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_visibility_is_written_as_the_character_it_is_read_from() {
        for (mark, visibility) in [
            ('+', Visibility::Public),
            ('-', Visibility::Private),
            ('#', Visibility::Protected),
            ('~', Visibility::Package),
        ] {
            assert_eq!(Visibility::from_mark(mark), Some(visibility));
            assert_eq!(visibility.mark(), mark.to_string());
        }
        assert_eq!(Visibility::from_mark('x'), None);
        assert_eq!(Visibility::default(), Visibility::Unstated);
        assert_eq!(Visibility::Unstated.mark(), "");
    }

    #[test]
    fn a_field_is_written_as_name_and_type() {
        let field = Member {
            visibility: Visibility::Public,
            name: "name".into(),
            kind: "String".into(),
            ..Member::default()
        };
        assert!(!field.is_method());
        assert_eq!(field.written(), "name");
        assert_eq!(field.line(), "+ name: String");
    }

    #[test]
    fn a_method_is_written_with_its_parentheses_even_when_they_are_empty() {
        let method = Member {
            visibility: Visibility::Private,
            name: "hash".into(),
            kind: "String".into(),
            params: Some(String::new()),
            ..Member::default()
        };
        assert!(method.is_method());
        assert_eq!(method.written(), "hash()");
        assert_eq!(method.line(), "- hash(): String");
    }

    #[test]
    fn a_member_with_nothing_written_about_it_is_just_its_name() {
        let bare = Member {
            name: "ACTIVE".into(),
            ..Member::default()
        };
        assert_eq!(bare.line(), "ACTIVE");
    }

    #[test]
    fn a_method_carries_its_parameters_into_the_box() {
        let method = Member {
            name: "setData".into(),
            params: Some("key, val".into()),
            ..Member::default()
        };
        assert_eq!(method.line(), "setData(key, val)");
    }

    #[test]
    fn every_relation_is_named_and_knows_how_it_is_drawn() {
        for relation in [
            Relation::Inheritance,
            Relation::Composition,
            Relation::Aggregation,
            Relation::Association,
            Relation::Dependency,
            Relation::Realization,
        ] {
            assert!(!relation.token().is_empty(), "{relation:?}");
            assert!(!relation.marker().is_empty(), "{relation:?}");
        }
        // The two "at a distance" relationships are the dashed ones.
        assert!(Relation::Dependency.dashed());
        assert!(Relation::Realization.dashed());
        assert!(!Relation::Inheritance.dashed());
        assert!(!Relation::Composition.dashed());
        assert!(!Relation::Aggregation.dashed());
        assert!(!Relation::Association.dashed());
        // And the pairs that share a marker are told apart by that line style.
        assert_eq!(
            Relation::Inheritance.marker(),
            Relation::Realization.marker()
        );
        assert_eq!(
            Relation::Association.marker(),
            Relation::Dependency.marker()
        );
        assert_ne!(
            Relation::Composition.marker(),
            Relation::Aggregation.marker()
        );
    }

    #[test]
    fn the_arrow_table_is_longest_first_so_no_arrow_hides_a_longer_one() {
        for (at, (arrow, _, _)) in ARROWS.iter().enumerate() {
            for (later, _, _) in ARROWS.iter().skip(at + 1) {
                assert!(
                    !arrow.starts_with(later) || arrow.len() >= later.len(),
                    "{later} comes after {arrow} but is a prefix of it"
                );
            }
        }
        // Every arrow reads as something, and both ways round are covered.
        assert_eq!(ARROWS.len(), 13);
    }

    #[test]
    fn a_diagram_finds_a_class_by_the_name_it_was_given() {
        let diagram = Diagram {
            classes: vec![
                Class {
                    id: "A".into(),
                    label: "A".into(),
                    ..Class::default()
                },
                Class {
                    id: "B".into(),
                    label: "B".into(),
                    ..Class::default()
                },
            ],
            relationships: Vec::new(),
        };
        assert_eq!(diagram.index_of("B"), Some(1));
        assert_eq!(diagram.index_of("C"), None);
    }
}
