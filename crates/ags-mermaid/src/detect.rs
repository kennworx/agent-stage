//! Which diagram a source is, decided once before anything else runs.
//!
//! Every later stage is dispatched on the answer, so nothing downstream has to
//! re-derive what it is holding.
//!
//! The renderer this replaces treats *anything* it does not recognise as a
//! flowchart. A misspelled header therefore reaches the flowchart parser, which
//! either fails obscurely or draws something that is not the diagram anyone
//! asked for. Here an unrecognised header is an error, and one edit away from a
//! real keyword is reported as the typo it almost certainly is.

/// A diagram kind this renderer knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagramType {
    Architecture,
    Block,
    C4,
    Class,
    Er,
    EventModeling,
    Flowchart,
    Gantt,
    GitGraph,
    Ishikawa,
    Journey,
    Kanban,
    Mindmap,
    Packet,
    Pie,
    Quadrant,
    Radar,
    Requirement,
    Sankey,
    Sequence,
    Timeline,
    Treemap,
    TreeView,
    Venn,
    Wardley,
    XyChart,
    ZenUml,
}

impl DiagramType {
    /// Every kind this renderer draws.
    ///
    /// The list an author is offered and the list Gate 1 accepts are both this
    /// one. It is exhaustive by construction: [`Self::keyword`] matches on
    /// `self` without a catch-all, so adding a variant without listing it here
    /// fails to compile there first.
    pub const ALL: [Self; 27] = [
        Self::Architecture,
        Self::Block,
        Self::C4,
        Self::Class,
        Self::Er,
        Self::EventModeling,
        Self::Flowchart,
        Self::Gantt,
        Self::GitGraph,
        Self::Ishikawa,
        Self::Journey,
        Self::Kanban,
        Self::Mindmap,
        Self::Packet,
        Self::Pie,
        Self::Quadrant,
        Self::Radar,
        Self::Requirement,
        Self::Sankey,
        Self::Sequence,
        Self::Timeline,
        Self::Treemap,
        Self::TreeView,
        Self::Venn,
        Self::Wardley,
        Self::XyChart,
        Self::ZenUml,
    ];

    /// The header an author writes to open this diagram.
    ///
    /// Spelled as it is written, not as it is looked up: [`KEYWORDS`] is keyed on
    /// a lowercased word, but an artifact says `classDiagram`, and a catalog that
    /// told an agent to write `classdiagram` would be teaching it the index
    /// rather than the language. Several kinds accept more spellings than this
    /// one — `graph` for a flowchart, four more `C4*` headers — so this is the
    /// canonical spelling, not the only accepted one.
    ///
    /// A round-trip test asserts every value here detects back to its own
    /// variant, which is what keeps the two tables from drifting apart.
    pub const fn keyword(self) -> &'static str {
        match self {
            Self::Architecture => "architecture",
            Self::Block => "block",
            Self::C4 => "C4Context",
            Self::Class => "classDiagram",
            Self::Er => "erDiagram",
            Self::EventModeling => "eventmodeling",
            Self::Flowchart => "flowchart",
            Self::Gantt => "gantt",
            Self::GitGraph => "gitGraph",
            Self::Ishikawa => "ishikawa",
            Self::Journey => "journey",
            Self::Kanban => "kanban",
            Self::Mindmap => "mindmap",
            Self::Packet => "packet",
            Self::Pie => "pie",
            Self::Quadrant => "quadrantChart",
            Self::Radar => "radar",
            Self::Requirement => "requirementDiagram",
            Self::Sankey => "sankey",
            Self::Sequence => "sequenceDiagram",
            Self::Timeline => "timeline",
            Self::Treemap => "treemap",
            Self::TreeView => "treeview",
            Self::Venn => "venn",
            Self::Wardley => "wardley",
            Self::XyChart => "xychart",
            Self::ZenUml => "zenuml",
        }
    }

    /// Whether this kind needs layered graph layout.
    ///
    /// These four are the reason a JavaScript renderer is still shipped: laying
    /// out an arbitrary directed graph is a different problem from placing a
    /// declared grid, and is not part of this port.
    pub const fn needs_layered_layout(self) -> bool {
        matches!(
            self,
            Self::Flowchart | Self::Class | Self::Er | Self::Architecture
        )
    }
}

/// Header keywords, and the kind each introduces.
///
/// Several diagrams are spelled with a `-beta` suffix in the wild; it is
/// stripped before lookup rather than doubling every entry.
const KEYWORDS: [(&str, DiagramType); 33] = [
    ("architecture", DiagramType::Architecture),
    ("block", DiagramType::Block),
    ("c4component", DiagramType::C4),
    ("c4container", DiagramType::C4),
    ("c4context", DiagramType::C4),
    ("c4deployment", DiagramType::C4),
    ("c4dynamic", DiagramType::C4),
    ("classdiagram", DiagramType::Class),
    ("erdiagram", DiagramType::Er),
    ("eventmodeling", DiagramType::EventModeling),
    ("flowchart", DiagramType::Flowchart),
    ("gantt", DiagramType::Gantt),
    ("gitgraph", DiagramType::GitGraph),
    ("graph", DiagramType::Flowchart),
    ("ishikawa", DiagramType::Ishikawa),
    ("journey", DiagramType::Journey),
    ("kanban", DiagramType::Kanban),
    ("mindmap", DiagramType::Mindmap),
    ("packet", DiagramType::Packet),
    ("pie", DiagramType::Pie),
    ("quadrantchart", DiagramType::Quadrant),
    ("radar", DiagramType::Radar),
    ("requirementdiagram", DiagramType::Requirement),
    ("sankey", DiagramType::Sankey),
    ("sequencediagram", DiagramType::Sequence),
    ("statediagram", DiagramType::Flowchart),
    ("timeline", DiagramType::Timeline),
    ("treemap", DiagramType::Treemap),
    ("treeview", DiagramType::TreeView),
    ("venn", DiagramType::Venn),
    ("wardley", DiagramType::Wardley),
    ("xychart", DiagramType::XyChart),
    ("zenuml", DiagramType::ZenUml),
];

/// The outcome of looking at a source's first line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detection {
    Known(DiagramType),
    /// Nothing matched. `suggestion` is set when the header is one edit from a
    /// real keyword, which is nearly always a typo rather than a new diagram.
    Unknown {
        found: String,
        suggestion: Option<&'static str>,
    },
}

/// The first line that carries anything, ignoring blanks and `%%` comments.
fn header_line(source: &str) -> Option<&str> {
    source
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with("%%"))
}

/// Suffixes a header may carry that do not change which diagram it is.
///
/// `-beta` marks a type still stabilising upstream and `-v2` a second grammar
/// for the same diagram; both are spelled in the wild and neither selects a
/// different renderer. Listed rather than pattern-matched, so an unrecognised
/// suffix stays unrecognised instead of being quietly discarded.
const VARIANT_SUFFIXES: [&str; 2] = ["-beta", "-v2"];

/// The leading word of a header, lowercased, with any variant suffix removed.
fn keyword_of(line: &str) -> String {
    let word = line
        .split(|c: char| c.is_whitespace() || c == ';')
        .next()
        .unwrap_or("");
    let lower = word.to_ascii_lowercase();
    for suffix in VARIANT_SUFFIXES {
        if let Some(base) = lower.strip_suffix(suffix) {
            return base.to_string();
        }
    }
    lower
}

/// Identify the diagram a source declares.
pub fn detect(source: &str) -> Detection {
    let Some(line) = header_line(source) else {
        return Detection::Unknown {
            found: String::new(),
            suggestion: None,
        };
    };
    let keyword = keyword_of(line);
    if let Some((_, kind)) = KEYWORDS.iter().find(|(k, _)| *k == keyword) {
        return Detection::Known(*kind);
    }
    Detection::Unknown {
        suggestion: nearest(&keyword),
        found: keyword,
    }
}

/// A keyword one edit away from `word`, if there is exactly one candidate.
///
/// Reported only when unambiguous: offering three guesses is noise, and picking
/// one arbitrarily would sometimes point at the wrong diagram.
fn nearest(word: &str) -> Option<&'static str> {
    let mut found: Option<&'static str> = None;
    for (keyword, _) in &KEYWORDS {
        if !one_edit_apart(word, keyword) {
            continue;
        }
        if found.is_some() {
            return None;
        }
        found = Some(keyword);
    }
    found
}

/// Whether `a` and `b` differ by a single insertion, deletion, substitution or
/// transposition.
///
/// Transposition is included deliberately. `sequecneDiagram` is two plain edits
/// from the real keyword but one transposition, and swapped letters are among
/// the most common ways to mistype a word — a rule that misses them misses the
/// case it exists for.
fn one_edit_apart(a: &str, b: &str) -> bool {
    let (av, bv): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    match av.len().abs_diff(bv.len()) {
        0 => one_substitution(&av, &bv) || one_transposition(&av, &bv),
        1 if av.len() > bv.len() => one_insertion(&bv, &av),
        1 => one_insertion(&av, &bv),
        _ => false,
    }
}

/// Equal lengths differing in exactly one position.
fn one_substitution(a: &[char], b: &[char]) -> bool {
    a.iter().zip(b).filter(|(x, y)| x != y).count() == 1
}

/// Equal lengths differing by two adjacent characters having swapped places.
fn one_transposition(a: &[char], b: &[char]) -> bool {
    let mut diffs = a.iter().zip(b).enumerate().filter(|(_, (x, y))| x != y);
    let (Some((i, (ai, bi))), Some((j, (aj, bj))), None) =
        (diffs.next(), diffs.next(), diffs.next())
    else {
        return false;
    };
    j == i + 1 && ai == bj && aj == bi
}

/// Whether `longer` is `shorter` with one character inserted.
fn one_insertion(shorter: &[char], longer: &[char]) -> bool {
    let mut i = 0;
    let mut skipped = false;
    for c in longer {
        if shorter.get(i) == Some(c) {
            i += 1;
        } else if skipped {
            return false;
        } else {
            skipped = true;
        }
    }
    i == shorter.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind(source: &str) -> DiagramType {
        match detect(source) {
            Detection::Known(k) => k,
            Detection::Unknown { found, suggestion } => {
                panic!("expected a known type, got unknown {found:?} (suggestion {suggestion:?})")
            }
        }
    }

    #[test]
    fn recognises_every_header_form() {
        assert_eq!(kind("pie showData title X"), DiagramType::Pie);
        assert_eq!(kind("C4Context"), DiagramType::C4);
        assert_eq!(kind("C4Deployment\n  Node(a)"), DiagramType::C4);
        assert_eq!(kind("sequenceDiagram"), DiagramType::Sequence);
        assert_eq!(kind("classDiagram"), DiagramType::Class);
        assert_eq!(kind("erDiagram"), DiagramType::Er);
        assert_eq!(kind("quadrantChart"), DiagramType::Quadrant);
        assert_eq!(kind("requirementDiagram"), DiagramType::Requirement);
        assert_eq!(kind("gitGraph"), DiagramType::GitGraph);
        assert_eq!(kind("zenuml"), DiagramType::ZenUml);
    }

    #[test]
    fn every_canonical_keyword_detects_back_to_its_own_kind() {
        // The anti-drift guard between `ALL`/`keyword()` — what an author is told
        // to write — and `KEYWORDS`, what the detector accepts. A catalog built
        // from the first is only trustworthy if the second agrees with it.
        for kind in DiagramType::ALL {
            assert_eq!(
                detect(kind.keyword()),
                Detection::Known(kind),
                "canonical keyword {:?} does not detect back to {kind:?}",
                kind.keyword()
            );
        }
    }

    #[test]
    fn the_list_names_every_kind_exactly_once() {
        let mut seen = std::collections::HashSet::new();
        for kind in DiagramType::ALL {
            assert!(seen.insert(kind), "{kind:?} listed twice in ALL");
        }
        // Every keyword in the lookup table reaches a kind that `ALL` names, so
        // no accepted header points at a kind the catalog never mentions.
        for (word, kind) in &KEYWORDS {
            assert!(
                seen.contains(kind),
                "{word} detects to {kind:?}, which ALL does not list"
            );
        }
    }

    #[test]
    fn a_beta_suffix_is_stripped() {
        assert_eq!(kind("xychart-beta"), DiagramType::XyChart);
        assert_eq!(kind("sankey-beta"), DiagramType::Sankey);
        assert_eq!(kind("packet-beta"), DiagramType::Packet);
        assert_eq!(kind("architecture-beta"), DiagramType::Architecture);
        assert_eq!(kind("sankey"), DiagramType::Sankey);
    }

    #[test]
    fn flowchart_has_several_spellings() {
        assert_eq!(kind("graph TD"), DiagramType::Flowchart);
        assert_eq!(kind("flowchart LR"), DiagramType::Flowchart);
        assert_eq!(kind("stateDiagram-v2"), DiagramType::Flowchart);
    }

    #[test]
    fn detection_ignores_blanks_and_comments() {
        assert_eq!(kind("\n\n  %% a note\n  pie title X"), DiagramType::Pie);
    }

    #[test]
    fn detection_is_case_insensitive() {
        assert_eq!(kind("PIE"), DiagramType::Pie);
        assert_eq!(kind("c4context"), DiagramType::C4);
    }

    #[test]
    fn an_unknown_header_is_not_silently_a_flowchart() {
        // The behaviour this replaces: anything unrecognised became a flowchart,
        // so a typo drew the wrong diagram instead of reporting itself.
        let Detection::Unknown { found, .. } = detect("sunburstChart") else {
            panic!("expected unknown");
        };
        assert_eq!(found, "sunburstchart");
    }

    #[test]
    fn a_near_miss_is_reported_as_a_typo() {
        for (typo, expected) in [
            ("pi", "pie"),
            ("pae", "pie"),
            ("piee", "pie"),
            ("mindmpa", "mindmap"),
            ("C4Contex", "c4context"),
        ] {
            let Detection::Unknown { suggestion, .. } = detect(typo) else {
                panic!("{typo} should not be a known type");
            };
            assert_eq!(suggestion, Some(expected), "for {typo}");
        }
    }

    #[test]
    fn a_transposition_counts_as_one_edit() {
        // Two edits by plain Levenshtein, one by the measure that matters — and
        // swapped letters are the commonest way to mistype a word.
        assert!(one_edit_apart("sequecnediagram", "sequencediagram"));
        assert!(one_edit_apart("gantt", "gnatt"));
    }

    #[test]
    fn an_ambiguous_near_miss_suggests_nothing() {
        // `treemap` and `treeview` are both one edit from neither, but a word
        // equidistant from two keywords must not pick one arbitrarily.
        assert_eq!(nearest("xxxxxxxxxxxx"), None);
    }

    #[test]
    fn a_distant_word_gets_no_suggestion() {
        let Detection::Unknown { suggestion, .. } = detect("completely-different") else {
            panic!("expected unknown");
        };
        assert_eq!(suggestion, None);
    }

    #[test]
    fn empty_input_is_unknown_rather_than_a_panic() {
        assert_eq!(
            detect(""),
            Detection::Unknown {
                found: String::new(),
                suggestion: None
            }
        );
        assert!(matches!(
            detect("   \n\n %% only a comment\n"),
            Detection::Unknown { .. }
        ));
    }

    /// Every kind this renderer names must be reachable from some header.
    ///
    /// A variant with no keyword would be undetectable and therefore dead — and
    /// silently so, since nothing else in the crate enumerates them.
    #[test]
    fn every_diagram_kind_has_a_header_that_selects_it() {
        for want in DiagramType::ALL {
            let reachable = KEYWORDS.iter().any(|(_, k)| *k == want);
            assert!(reachable, "{want:?} has no header keyword");
        }
    }

    #[test]
    fn the_four_deferred_types_are_the_ones_needing_layered_layout() {
        for kind in [
            DiagramType::Flowchart,
            DiagramType::Class,
            DiagramType::Er,
            DiagramType::Architecture,
        ] {
            assert!(kind.needs_layered_layout(), "{kind:?}");
        }
        for kind in [
            DiagramType::C4,
            DiagramType::Pie,
            DiagramType::Sequence,
            DiagramType::XyChart,
            DiagramType::Sankey,
        ] {
            assert!(!kind.needs_layered_layout(), "{kind:?}");
        }
    }
}
