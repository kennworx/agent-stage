//! The parsed shape of an `architecture-beta` diagram.
//!
//! Three things are declared — groups, services and junctions — and they differ
//! only in what they are drawn as, so they are one record with a kind rather
//! than three lists. That keeps declaration order, which is the tie-break the
//! placement falls back on, in one obvious place.

/// Which side of a box an edge meets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    /// The letter this is written with.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Left => "L",
            Self::Right => "R",
            Self::Top => "T",
            Self::Bottom => "B",
        }
    }

    /// The side a letter names, in whatever case it was written.
    pub fn from_letter(letter: char) -> Option<Self> {
        match letter.to_ascii_uppercase() {
            'L' => Some(Self::Left),
            'R' => Some(Self::Right),
            'T' => Some(Self::Top),
            'B' => Some(Self::Bottom),
            _ => None,
        }
    }

    /// Which way this side faces, in whole cells.
    ///
    /// This is what makes the side letters a placement language rather than a
    /// routing hint: `a:R -- L:b` does not say "leave on the right", it says
    /// "b is to the right of a".
    pub const fn step(self) -> (i64, i64) {
        match self {
            Self::Left => (-1, 0),
            Self::Right => (1, 0),
            Self::Top => (0, -1),
            Self::Bottom => (0, 1),
        }
    }

    pub const fn opposite(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Top => Self::Bottom,
            Self::Bottom => Self::Top,
        }
    }

    /// Whether an edge leaves this side sideways rather than up or down.
    pub const fn across(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// What a declared thing is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A labelled container round other things.
    Group,
    /// A box with a glyph and a name.
    Service,
    /// A dot where several lines meet.
    Junction,
}

/// One declared thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub kind: Kind,
    /// The glyph named in parentheses. Empty when none was.
    pub icon: String,
    /// What is drawn on it; the id when nothing was written.
    pub title: String,
    /// The group this sits in, by id. Empty when it sits at the top.
    pub parent: String,
}

/// One line between two declared things.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub from: String,
    /// Which side of `from` this leaves. Unstated means "whichever faces `to`".
    pub from_side: Option<Side>,
    pub to: String,
    pub to_side: Option<Side>,
    pub arrow_start: bool,
    pub arrow_end: bool,
}

/// A parsed architecture diagram.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diagram {
    /// Everything declared, in the order it was declared.
    pub items: Vec<Item>,
    pub edges: Vec<Edge>,
}

impl Diagram {
    /// Where `id` sits in `items`.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.items.iter().position(|item| item.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_side_is_read_from_the_letter_it_is_written_as() {
        for (letter, side) in [
            ('L', Side::Left),
            ('r', Side::Right),
            ('T', Side::Top),
            ('b', Side::Bottom),
        ] {
            assert_eq!(Side::from_letter(letter), Some(side));
        }
        assert_eq!(Side::from_letter('x'), None);
        assert_eq!(Side::Left.token(), "L");
        assert_eq!(Side::Right.token(), "R");
        assert_eq!(Side::Top.token(), "T");
        assert_eq!(Side::Bottom.token(), "B");
    }

    #[test]
    fn a_side_faces_away_from_the_box_it_belongs_to() {
        assert_eq!(Side::Left.step(), (-1, 0));
        assert_eq!(Side::Right.step(), (1, 0));
        assert_eq!(Side::Top.step(), (0, -1));
        assert_eq!(Side::Bottom.step(), (0, 1));
    }

    #[test]
    fn opposite_sides_face_opposite_ways_and_come_back_round() {
        for side in [Side::Left, Side::Right, Side::Top, Side::Bottom] {
            assert_eq!(side.opposite().opposite(), side);
            let (dx, dy) = side.step();
            assert_eq!(side.opposite().step(), (-dx, -dy));
            assert_eq!(side.across(), side.opposite().across());
        }
        assert!(Side::Left.across());
        assert!(!Side::Top.across());
    }

    #[test]
    fn a_diagram_finds_a_thing_by_the_name_it_was_given() {
        let diagram = Diagram {
            items: vec![
                Item {
                    id: "cloud".into(),
                    kind: Kind::Group,
                    icon: "cloud".into(),
                    title: "Cloud".into(),
                    parent: String::new(),
                },
                Item {
                    id: "web".into(),
                    kind: Kind::Service,
                    icon: "server".into(),
                    title: "Web".into(),
                    parent: "cloud".into(),
                },
            ],
            edges: Vec::new(),
        };
        assert_eq!(diagram.index_of("web"), Some(1));
        assert_eq!(diagram.index_of("db"), None);
    }
}
