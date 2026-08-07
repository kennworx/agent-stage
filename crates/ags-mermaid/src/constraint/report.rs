//! What a broken rule is called, and how it reads.

/// A legibility rule that a drawing broke.
#[derive(Debug, Clone, PartialEq)]
pub enum Violation {
    /// Something is drawn outside the canvas, where it is cut rather than shrunk.
    OutsideCanvas { id: Option<String> },
    /// An edge passes through a box it does not connect.
    EdgeThroughNode {
        edge: Option<String>,
        node: Option<String>,
    },
    /// Two edges are drawn along the same line, and read as one.
    MergedEdges {
        a: Option<String>,
        b: Option<String>,
        length: f64,
    },
    /// A label is completely covered by something painted after it.
    Occluded { id: Option<String> },
    /// Two edges cross, so a reader tracing one can leave on the other.
    EdgesCross {
        a: Option<String>,
        b: Option<String>,
    },
    /// An edge leaves a box by the face pointing away from where it is going, so
    /// the drawing sends the eye in the wrong direction before the line turns.
    WrongFace {
        edge: Option<String>,
        node: Option<String>,
    },
    /// A route travels away from its target before coming back, so its length
    /// says the two things are further apart than they are.
    Backtracks { edge: Option<String>, by: f64 },
    /// A frame is drawn round a box that does not belong to it, so the drawing
    /// says something the source did not.
    ///
    /// Geometry cannot infer this: a frame is drawn round wherever its members
    /// ended up, and whether a stranger fell inside is only knowable from the
    /// membership the source declared. A frame that claims a `holds` datum is
    /// asking to be checked against it.
    Enclosed {
        frame: Option<String>,
        node: Option<String>,
    },
}

/// A violation as a sentence, because the only thing that ever reads one is a
/// person deciding whether the drawing is good enough.
impl std::fmt::Display for Violation {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = |id: &Option<String>| id.clone().unwrap_or_else(|| "something".to_string());
        match self {
            Self::OutsideCanvas { id } => {
                write!(out, "{} is drawn outside the canvas", name(id))
            }
            Self::EdgeThroughNode { edge, node } => write!(
                out,
                "the edge {} passes through {}, which it does not connect",
                name(edge),
                name(node)
            ),
            Self::MergedEdges { a, b, length } => write!(
                out,
                "the edges {} and {} share {length:.0}px of one line, and read as a single edge",
                name(a),
                name(b)
            ),
            Self::Occluded { id } => write!(
                out,
                "the label {} is completely covered by something drawn after it",
                name(id)
            ),
            Self::EdgesCross { a, b } => write!(
                out,
                "the edges {} and {} cross, so a reader tracing one can leave on the other",
                name(a),
                name(b)
            ),
            Self::WrongFace { edge, node } => write!(
                out,
                "the edge {} leaves {} by the face pointing away from where it goes",
                name(edge),
                name(node)
            ),
            Self::Backtracks { edge, by } => write!(
                out,
                "the edge {} travels {by:.0}px away from its target before turning back",
                name(edge)
            ),
            Self::Enclosed { frame, node } => write!(
                out,
                "the frame {} is drawn round {}, which is not in it",
                name(frame),
                name(node)
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_violation_reads_as_a_sentence() {
        let said = |violation: &Violation| violation.to_string();
        assert!(said(&Violation::OutsideCanvas {
            id: Some("A".into())
        })
        .contains('A'));
        assert!(said(&Violation::OutsideCanvas { id: None }).contains("something"));
        assert!(said(&Violation::EdgeThroughNode {
            edge: Some("e1".into()),
            node: Some("B".into()),
        })
        .contains("passes through"));
        assert!(said(&Violation::MergedEdges {
            a: Some("e1".into()),
            b: Some("e2".into()),
            length: 42.0,
        })
        .contains("42px"));
        assert!(said(&Violation::Occluded {
            id: Some("L".into())
        })
        .contains("covered"));
        assert!(said(&Violation::EdgesCross {
            a: Some("e1".into()),
            b: None,
        })
        .contains("e1 and something cross"));
        assert!(said(&Violation::WrongFace {
            edge: Some("e1".into()),
            node: Some("A".into()),
        })
        .contains("leaves A by the face"));
        assert!(said(&Violation::Backtracks {
            edge: Some("e1".into()),
            by: 61.4,
        })
        .contains("61px"));
    }

    #[test]
    fn an_enclosure_reads_as_a_sentence() {
        let said = Violation::Enclosed {
            frame: Some("ci".into()),
            node: Some("E".into()),
        }
        .to_string();
        assert_eq!(said, "the frame ci is drawn round E, which is not in it");
        let anonymous = Violation::Enclosed {
            frame: None,
            node: None,
        }
        .to_string();
        assert!(anonymous.contains("something"), "{anonymous}");
    }
}
