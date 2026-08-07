//! The parsed shape of an entity-relationship diagram.
//!
//! The only thing here that is not a plain record is [`Cardinality`], and it
//! carries its own drawing rules — how many bars, whether a foot, whether a
//! ring. Crow's-foot notation is a small alphabet where each glyph means
//! something exact, so the mapping from notation to glyph is written once, in
//! the type, rather than in the renderer where a missing bar would read as a
//! different constraint.

/// How many of one entity take part, at one end of a relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cardinality {
    /// `||` — exactly one.
    One,
    /// `|o` — one, or none.
    ZeroOne,
    /// `}|` — one or more.
    Many,
    /// `o{` — any number, including none.
    ZeroMany,
}

impl Cardinality {
    /// The name this is known by, which becomes its datum.
    pub const fn token(self) -> &'static str {
        match self {
            Self::One => "one",
            Self::ZeroOne => "zero-one",
            Self::Many => "many",
            Self::ZeroMany => "zero-many",
        }
    }

    /// How many bars cross the line. Two say "exactly one"; one says "one" as
    /// part of a larger claim; none is left to the ring and the foot.
    pub const fn bars(self) -> usize {
        match self {
            Self::One => 2,
            Self::ZeroOne | Self::Many => 1,
            Self::ZeroMany => 0,
        }
    }

    /// Whether the three-line foot is drawn, which is what says "more than one".
    pub const fn toes(self) -> bool {
        matches!(self, Self::Many | Self::ZeroMany)
    }

    /// Whether the ring is drawn, which is what says "possibly none".
    pub const fn ring(self) -> bool {
        matches!(self, Self::ZeroOne | Self::ZeroMany)
    }

    /// The cardinality a notation names, whichever way round it was written.
    ///
    /// `|o` and `o|` are the same claim seen from either side of the line, so
    /// the characters are sorted before they are read.
    pub fn from_notation(text: &str) -> Option<Self> {
        let mut marks: Vec<char> = text.chars().collect();
        marks.sort_unstable();
        match marks.as_slice() {
            ['|', '|'] => Some(Self::One),
            ['o', '|'] => Some(Self::ZeroOne),
            ['{', '|'] | ['|', '}'] => Some(Self::Many),
            ['o', '{'] => Some(Self::ZeroMany),
            _ => None,
        }
    }
}

/// A constraint written after an attribute's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Primary,
    Foreign,
    Unique,
}

impl Key {
    /// The badge this is drawn as.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Primary => "PK",
            Self::Foreign => "FK",
            Self::Unique => "UK",
        }
    }

    /// The key a word names, in whatever case it was written.
    pub fn from_word(word: &str) -> Option<Self> {
        match word.to_ascii_uppercase().as_str() {
            "PK" => Some(Self::Primary),
            "FK" => Some(Self::Foreign),
            "UK" => Some(Self::Unique),
            _ => None,
        }
    }
}

/// One row inside an entity: a column.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Attribute {
    /// The data type, which is written first.
    pub kind: String,
    pub name: String,
    pub keys: Vec<Key>,
    /// A quoted note, shown as hover text rather than drawn.
    pub comment: String,
}

impl Attribute {
    /// The keys as they are written together on the badge.
    pub fn badge(&self) -> String {
        self.keys
            .iter()
            .map(|key| key.token())
            .collect::<Vec<&str>>()
            .join(",")
    }

    /// The row as one string. Only sizing needs this — the parts are drawn
    /// separately, at opposite ends of the row.
    pub fn line(&self) -> String {
        let badge = self.badge();
        if badge.is_empty() {
            return format!("{}  {}", self.kind, self.name);
        }
        format!("{}  {}  {badge}", self.kind, self.name)
    }
}

/// One box.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Entity {
    pub id: String,
    pub label: String,
    pub attributes: Vec<Attribute>,
}

/// One line between two boxes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub from: String,
    pub to: String,
    pub from_cardinality: Cardinality,
    pub to_cardinality: Cardinality,
    /// The verb, which reads left to right along the line.
    pub label: String,
    /// `--` rather than `..`: the child cannot exist without the parent, and
    /// the line is drawn solid to say so.
    pub identifying: bool,
}

/// A parsed ER diagram.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diagram {
    /// Entities in the order they were first named.
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
}

impl Diagram {
    /// Where `id` sits in `entities`.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.entities.iter().position(|entity| entity.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_notation_is_read_whichever_way_round_it_was_written() {
        for (notation, expected) in [
            ("||", Cardinality::One),
            ("|o", Cardinality::ZeroOne),
            ("o|", Cardinality::ZeroOne),
            ("}|", Cardinality::Many),
            ("|{", Cardinality::Many),
            ("o{", Cardinality::ZeroMany),
            ("{o", Cardinality::ZeroMany),
        ] {
            assert_eq!(
                Cardinality::from_notation(notation),
                Some(expected),
                "{notation}"
            );
        }
        assert_eq!(Cardinality::from_notation("||||"), None);
        assert_eq!(Cardinality::from_notation("x"), None);
        assert_eq!(Cardinality::from_notation(""), None);
    }

    #[test]
    fn each_cardinality_is_drawn_as_the_glyphs_that_say_what_it_means() {
        // Exactly one: two bars, nothing else.
        assert_eq!(Cardinality::One.bars(), 2);
        assert!(!Cardinality::One.toes());
        assert!(!Cardinality::One.ring());
        // One or none: a bar, and a ring for the "or none".
        assert_eq!(Cardinality::ZeroOne.bars(), 1);
        assert!(!Cardinality::ZeroOne.toes());
        assert!(Cardinality::ZeroOne.ring());
        // One or more: a foot for the "more", a bar for the "one".
        assert_eq!(Cardinality::Many.bars(), 1);
        assert!(Cardinality::Many.toes());
        assert!(!Cardinality::Many.ring());
        // Any number: a foot and a ring, and no claim of one.
        assert_eq!(Cardinality::ZeroMany.bars(), 0);
        assert!(Cardinality::ZeroMany.toes());
        assert!(Cardinality::ZeroMany.ring());
        for card in [
            Cardinality::One,
            Cardinality::ZeroOne,
            Cardinality::Many,
            Cardinality::ZeroMany,
        ] {
            assert!(!card.token().is_empty(), "{card:?}");
        }
    }

    #[test]
    fn every_key_is_read_from_the_word_it_is_written_as() {
        assert_eq!(Key::from_word("PK"), Some(Key::Primary));
        assert_eq!(Key::from_word("fk"), Some(Key::Foreign));
        assert_eq!(Key::from_word("Uk"), Some(Key::Unique));
        assert_eq!(Key::from_word("XK"), None);
        assert_eq!(Key::Primary.token(), "PK");
        assert_eq!(Key::Foreign.token(), "FK");
        assert_eq!(Key::Unique.token(), "UK");
    }

    #[test]
    fn an_attribute_writes_its_keys_together_on_one_badge() {
        let attribute = Attribute {
            kind: "int".into(),
            name: "id".into(),
            keys: vec![Key::Primary, Key::Foreign],
            comment: String::new(),
        };
        assert_eq!(attribute.badge(), "PK,FK");
        assert_eq!(attribute.line(), "int  id  PK,FK");
    }

    #[test]
    fn an_attribute_with_no_keys_has_no_badge() {
        let attribute = Attribute {
            kind: "string".into(),
            name: "email".into(),
            ..Attribute::default()
        };
        assert_eq!(attribute.badge(), "");
        assert_eq!(attribute.line(), "string  email");
    }

    #[test]
    fn a_diagram_finds_an_entity_by_the_name_it_was_given() {
        let diagram = Diagram {
            entities: vec![
                Entity {
                    id: "CUSTOMER".into(),
                    label: "CUSTOMER".into(),
                    attributes: Vec::new(),
                },
                Entity {
                    id: "ORDER".into(),
                    label: "ORDER".into(),
                    attributes: Vec::new(),
                },
            ],
            relationships: Vec::new(),
        };
        assert_eq!(diagram.index_of("ORDER"), Some(1));
        assert_eq!(diagram.index_of("INVOICE"), None);
    }
}
