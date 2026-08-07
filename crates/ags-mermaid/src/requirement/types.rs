//! The parsed shape of a requirement diagram: requirements, elements, and the
//! typed relationships between them.

/// The keyword a requirement block opens with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    Requirement,
    Functional,
    Interface,
    Performance,
    Physical,
    DesignConstraint,
}

impl Kind {
    /// The keyword itself, as written.
    /// Every kind, so a test over the tables below can be exhaustive by
    /// construction rather than by someone remembering to add a line.
    pub const ALL: [Self; 6] = [
        Self::Requirement,
        Self::Functional,
        Self::Interface,
        Self::Performance,
        Self::Physical,
        Self::DesignConstraint,
    ];

    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Requirement => "requirement",
            Self::Functional => "functionalRequirement",
            Self::Interface => "interfaceRequirement",
            Self::Performance => "performanceRequirement",
            Self::Physical => "physicalRequirement",
            Self::DesignConstraint => "designConstraint",
        }
    }

    /// The `«Stereotype»` written under a box's name: the keyword split at its
    /// humps and title-cased.
    pub fn stereotype(self) -> String {
        let mut words = String::new();
        for (i, c) in self.keyword().chars().enumerate() {
            if c.is_ascii_uppercase() && i > 0 {
                words.push(' ');
            }
            words.push(if i == 0 { c.to_ascii_uppercase() } else { c });
        }
        format!("«{words}»")
    }

    /// The keyword, if it is one.
    pub fn from_keyword(word: &str) -> Option<Self> {
        match word {
            "requirement" => Some(Self::Requirement),
            "functionalRequirement" => Some(Self::Functional),
            "interfaceRequirement" => Some(Self::Interface),
            "performanceRequirement" => Some(Self::Performance),
            "physicalRequirement" => Some(Self::Physical),
            "designConstraint" => Some(Self::DesignConstraint),
            _ => None,
        }
    }
}

/// A parsed requirement diagram.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Diagram {
    pub requirements: Vec<Requirement>,
    pub elements: Vec<Element>,
    pub relationships: Vec<Relationship>,
}

/// One `requirement` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    /// The name, which is also its identity.
    pub name: String,
    pub kind: Kind,
    pub id: Option<String>,
    pub text: Option<String>,
    pub risk: Option<String>,
    pub verify_method: Option<String>,
}

/// One `element` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub kind: Option<String>,
    pub docref: Option<String>,
}

/// One typed relationship. Always stored source-to-destination, whichever way
/// round it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub source: String,
    pub kind: String,
    pub dest: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_keyword_reads_back_as_the_kind_that_wrote_it() {
        // The round trip is the property that matters: the parser looks a word up
        // here, and the renderer writes one back out.
        for kind in Kind::ALL {
            assert_eq!(
                Kind::from_keyword(kind.keyword()),
                Some(kind),
                "{} does not read back",
                kind.keyword()
            );
        }
    }

    #[test]
    fn no_two_kinds_share_a_keyword() {
        let mut seen: Vec<&str> = Kind::ALL.iter().map(|k| k.keyword()).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), Kind::ALL.len());
    }

    #[test]
    fn a_stereotype_splits_the_keyword_at_its_humps() {
        assert_eq!(Kind::Requirement.stereotype(), "«Requirement»");
        assert_eq!(Kind::Functional.stereotype(), "«Functional Requirement»");
        assert_eq!(Kind::DesignConstraint.stereotype(), "«Design Constraint»");
        // Every kind gets one, and it is never empty guillemets.
        for kind in Kind::ALL {
            let written = kind.stereotype();
            assert!(
                written.starts_with('«') && written.ends_with('»'),
                "{written}"
            );
            assert!(written.chars().count() > 2, "{written}");
        }
    }

    #[test]
    fn a_word_that_is_not_a_keyword_names_no_kind() {
        assert_eq!(Kind::from_keyword("requirements"), None);
        assert_eq!(Kind::from_keyword(""), None);
        assert_eq!(
            Kind::from_keyword("Requirement"),
            None,
            "matched as written"
        );
    }
}
