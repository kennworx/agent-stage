//! Sizing the entity boxes, handing the graph to the engine, and working out
//! where each crow's foot goes.
//!
//! The feet are geometry, not drawing, so they are computed here where they can
//! be asserted against without rendering anything. Each is a set of bars, a
//! three-line foot and a ring, laid back along the line from the box it touches
//! — measured in that order so the glyphs never sit on top of one another
//! whichever combination a cardinality asks for.

use crate::label::{beside, runs as label_runs, Placed as PlacedLabel};
use crate::layout;
use crate::metrics::{mono_text_width, text_width};
use crate::scene::Point;

use super::types::{Attribute, Cardinality, Diagram, Entity};

/// Inside a box, either side of its rows.
pub const PAD_X: f64 = 14.0;
/// The band at the top holding the entity's name.
pub const HEADER_HEIGHT: f64 = 34.0;
/// One column.
pub const ROW_HEIGHT: f64 = 22.0;
pub const MIN_WIDTH: f64 = 140.0;
pub const ROW_FONT: f64 = 11.0;
pub const ROW_WEIGHT: u32 = 400;
pub const NAME_FONT: f64 = 13.0;
/// The entity name is drawn bold, so it is measured bold.
pub const NAME_WEIGHT: u32 = 700;
pub const KEY_FONT: f64 = 9.0;
pub const KEY_WEIGHT: u32 = 600;
pub const LABEL_FONT: f64 = 11.0;
pub const LABEL_WEIGHT: u32 = 400;
/// A line of text is this much taller than the type it is set in.
pub const LINE_HEIGHT: f64 = 1.3;
/// Between two boxes in one layer.
pub const NODE_GAP: f64 = 70.0;
/// Between one layer and the next. Wide, because both ends of every line carry
/// a glyph two dozen pixels long.
pub const LAYER_GAP: f64 = 90.0;
/// Around the whole drawing.
pub const PADDING: f64 = 40.0;
/// How far a verb sits clear of the line it names.
pub const LABEL_GAP: f64 = 6.0;

/// How far from the box the first glyph of a foot sits.
pub const FOOT_TIP: f64 = 4.0;
/// How far back the three lines of the foot converge.
pub const FOOT_BACK: f64 = 16.0;
/// Half the length of a bar across the line.
pub const BAR_HALF: f64 = 6.0;
/// How far the outer lines of the foot spread at the box.
pub const FAN_HALF: f64 = 7.0;
/// Between one bar and the next.
pub const BAR_GAP: f64 = 4.0;
pub const RING_RADIUS: f64 = 4.0;
/// Where the ring sits when it is the only glyph past the bar.
pub const RING_NEAR: f64 = 12.0;
/// And where it sits when a foot comes first, so it rests on the foot's point.
pub const RING_FAR: f64 = 20.0;

/// One box, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedEntity {
    pub id: String,
    pub label: String,
    pub attributes: Vec<Attribute>,
    pub at: Point,
    pub width: f64,
    pub height: f64,
    pub header: f64,
}

/// The glyphs at one end of a relationship, in diagram coordinates.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Foot {
    /// Bars across the line.
    pub bars: Vec<(Point, Point)>,
    /// The three lines that fan out towards the box.
    pub toes: Vec<(Point, Point)>,
    /// The ring that says "or none".
    pub ring: Option<Point>,
}

/// One relationship, routed, with a foot at each end.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedRelationship {
    pub from: String,
    pub to: String,
    pub from_cardinality: Cardinality,
    pub to_cardinality: Cardinality,
    pub label: String,
    pub identifying: bool,
    pub points: Vec<Point>,
    pub feet: Vec<Foot>,
    /// Where the label sits, when there is one.
    pub label_at: Option<Point>,
}

/// A laid-out ER diagram.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub entities: Vec<PlacedEntity>,
    pub relationships: Vec<PlacedRelationship>,
}

/// The engine's coordinates in the scene's own type.
const fn at(point: layout::Point) -> Point {
    Point::new(point.x, point.y)
}

/// How wide the badge holding an attribute's keys is.
pub fn badge_width(attribute: &Attribute) -> f64 {
    let badge = attribute.badge();
    if badge.is_empty() {
        return 0.0;
    }
    text_width(&badge, KEY_FONT, KEY_WEIGHT) + 8.0
}

/// How big an entity box has to be to hold what is in it.
///
/// One row of height even with nothing in it, because a box with a name and no
/// body reads as an unfinished drawing rather than as an entity nobody has
/// listed the columns of yet.
pub fn measure(entity: &Entity) -> layout::Node {
    let name = text_width(&entity.label, NAME_FONT, NAME_WEIGHT);
    let rows = entity
        .attributes
        .iter()
        .map(|attribute| mono_text_width(&attribute.line(), ROW_FONT))
        .fold(0.0, f64::max);
    let width = MIN_WIDTH.max(name + PAD_X * 2.0).max(rows + PAD_X * 2.0);
    let count = layout::as_f64(entity.attributes.len().max(1));
    layout::Node::new(width, HEADER_HEIGHT + count * ROW_HEIGHT)
}

/// A unit vector from `from` towards `at`, and the one at right angles to it.
fn along(at: Point, from: Point) -> Option<(Point, Point)> {
    let (dx, dy) = (at.x - from.x, at.y - from.y);
    let len = dx.hypot(dy);
    if len < 1e-9 {
        return None;
    }
    let unit = Point::new(dx / len, dy / len);
    Some((unit, Point::new(-unit.y, unit.x)))
}

/// The crow's foot at one end of a line.
///
/// `at` is the point on the box; `from` is where the line came from, which is
/// all that is needed to know which way the glyphs face. Everything is measured
/// back from `at`, so a foot never reaches into the box it labels.
pub fn foot(at: Point, from: Point, card: Cardinality) -> Foot {
    let Some((unit, across)) = along(at, from) else {
        return Foot::default();
    };
    let back = |distance: f64| Point::new(at.x - unit.x * distance, at.y - unit.y * distance);
    let span = |centre: Point, half: f64| {
        (
            Point::new(centre.x + across.x * half, centre.y + across.y * half),
            Point::new(centre.x - across.x * half, centre.y - across.y * half),
        )
    };
    // A foot occupies the near end of the line, so anything else starts past it.
    let first_bar = if card.toes() {
        FOOT_BACK + BAR_GAP
    } else {
        FOOT_TIP
    };
    let bars = (0..card.bars())
        .map(|index| span(back(first_bar + layout::as_f64(index) * BAR_GAP), BAR_HALF))
        .collect();
    let toes = if card.toes() {
        let (top, bottom) = span(back(FOOT_TIP), FAN_HALF);
        let point = back(FOOT_BACK);
        vec![(top, point), (back(FOOT_TIP), point), (bottom, point)]
    } else {
        Vec::new()
    };
    Foot {
        bars,
        toes,
        ring: card
            .ring()
            .then(|| back(if card.toes() { RING_FAR } else { RING_NEAR })),
    }
}

/// Both feet of a routed relationship, the first end first.
fn feet(points: &[Point], from: Cardinality, to: Cardinality) -> Vec<Foot> {
    let count = points.len();
    if count < 2 {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let (Some(first), Some(second)) = (points.first(), points.get(1)) {
        out.push(foot(*first, *second, from));
    }
    if let (Some(last), Some(before)) = (points.last(), points.get(count - 2)) {
        out.push(foot(*last, *before, to));
    }
    out
}

/// How wide and tall a verb is, over however many lines it takes.
pub fn label_size(label: &str) -> (f64, f64) {
    let lines: Vec<&str> = label.split('\n').collect();
    let width = lines
        .iter()
        .map(|text| text_width(text, LABEL_FONT, LABEL_WEIGHT))
        .fold(0.0, f64::max);
    (
        width,
        layout::as_f64(lines.len().max(1)) * LABEL_FONT * LINE_HEIGHT,
    )
}

/// How much room the verbs need between one layer and the next.
///
/// The engine reserves nothing for an edge's label, and both ends of every line
/// here already carry a glyph two dozen pixels long. Without this the verb is
/// drawn over the crow's feet it sits between. Widening every gap by the widest
/// verb is blunt — a per-edge reservation belongs in the engine — but it is
/// honest about what it costs and it never overlaps.
fn label_room(diagram: &Diagram) -> f64 {
    diagram
        .relationships
        .iter()
        .map(|rel| label_size(&rel.label).0)
        .fold(0.0, f64::max)
}

/// The straight runs of a route, each with its length.
fn runs(points: &[Point]) -> Vec<(Point, Point, f64)> {
    points
        .windows(2)
        .filter_map(|pair| {
            let (a, b) = (pair.first()?, pair.get(1)?);
            Some((*a, *b, (b.x - a.x).hypot(b.y - a.y)))
        })
        .collect()
}

/// The point half way along a route, whether the run it falls in is upright,
/// and which run that is.
///
/// Half way along the *line*, not the middle of the bounding box, which for an
/// L-shaped route is a place the line never goes.
pub fn midpoint(points: &[Point]) -> Option<(Point, bool, usize)> {
    let legs = runs(points);
    let total: f64 = legs.iter().map(|(_, _, len)| len).sum();
    if total < 1e-9 {
        return points.first().map(|only| (*only, false, 0));
    }
    let half = total / 2.0;
    let mut walked = 0.0;
    // The run the halfway mark falls in. There is always one, because `walked`
    // ends at `total` and the mark is half of it.
    let (at, (a, b, len)) = legs.into_iter().enumerate().find(|(_, (_, _, len))| {
        walked += len;
        walked >= half
    })?;
    // Whatever this run overshot by, taken back off it. `len` cannot be nought:
    // a run of no length cannot be the one that carried `walked` past the mark.
    let step = (len - (walked - half)) / len;
    Some((
        Point::new(a.x + (b.x - a.x) * step, a.y + (b.y - a.y) * step),
        (b.y - a.y).abs() > (b.x - a.x).abs(),
        at,
    ))
}

/// Every leg of a verb's own route except the one it is anchored to.
///
/// A verb has to sit beside its own line to read as its label, so the leg it is
/// anchored to cannot be an obstacle. Its route's *other* legs are a different
/// matter: they are lines like any other, and treating the whole route as
/// exempt is what drew `receives` and `generates` with their own turn struck
/// through them. An elbow puts the halfway mark near the corner, so the leg
/// round that corner is precisely the one in the way.
fn own_legs(points: &[Point], anchored: usize) -> Vec<(Point, Point)> {
    label_runs(points)
        .into_iter()
        .enumerate()
        .filter(|(at, _)| *at != anchored)
        .map(|(_, leg)| leg)
        .collect()
}

/// Where a verb sits, given everything already placed.
///
/// `taken` keeps two verbs apart; `lines` keeps a verb off any line it does not
/// belong beside — every other route, and every leg of its own but the one it
/// is anchored to.
fn label_at(
    points: &[Point],
    label: &str,
    taken: &[PlacedLabel],
    lines: &[(Point, Point)],
) -> Option<PlacedLabel> {
    let (middle, upright, anchored) = midpoint(points)?;
    let mut lines = lines.to_vec();
    lines.extend(own_legs(points, anchored));
    Some(beside(
        middle,
        upright,
        label_size(label),
        LABEL_GAP,
        taken,
        &lines,
    ))
}

/// Every run of every route but `mine`, which a verb has to keep off.
fn elsewhere(routes: &[Vec<Point>], mine: usize) -> Vec<(Point, Point)> {
    routes
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != mine)
        .flat_map(|(_, points)| label_runs(points))
        .collect()
}

/// Lay out a parsed ER diagram.
pub fn layout(diagram: &Diagram) -> Placed {
    if diagram.entities.is_empty() {
        return Placed::default();
    }
    let nodes: Vec<layout::Node> = diagram.entities.iter().map(measure).collect();
    let edges: Vec<layout::Edge> = diagram
        .relationships
        .iter()
        .map(|rel| {
            layout::Edge::new(
                diagram.index_of(&rel.from).unwrap_or(usize::MAX),
                diagram.index_of(&rel.to).unwrap_or(usize::MAX),
            )
        })
        .collect();
    let placed = layout::layout(&layout::Graph {
        nodes,
        edges,
        // Across the page rather than down it: an entity box is wide and short,
        // and a column of them wastes the width every one of them needs.
        direction: layout::Direction::Right,
        spacing: layout::Spacing {
            node: NODE_GAP,
            layer: LAYER_GAP + label_room(diagram),
            padding: PADDING,
            ..layout::Spacing::default()
        },
        ports: Vec::new(),
    });

    let entities: Vec<PlacedEntity> = diagram
        .entities
        .iter()
        .zip(&placed.nodes)
        .map(|(entity, node)| PlacedEntity {
            id: entity.id.clone(),
            label: entity.label.clone(),
            attributes: entity.attributes.clone(),
            at: at(node.at),
            width: node.width,
            height: node.height,
            header: HEADER_HEIGHT,
        })
        .collect();

    // Every route first, because a verb has to know where the other lines run
    // before it can keep off them.
    let routes: Vec<Vec<Point>> = placed
        .edges
        .iter()
        .map(|route| route.points.iter().copied().map(at).collect())
        .collect();
    // Seeded with the boxes, because a verb pushed off a line and into an
    // entity has not been helped. It then grows as the verbs are placed, so
    // each keeps out of the way of the ones before it.
    let mut taken: Vec<PlacedLabel> = entities
        .iter()
        .map(|entity| {
            PlacedLabel::new(
                Point::new(
                    entity.at.x + entity.width / 2.0,
                    entity.at.y + entity.height / 2.0,
                ),
                entity.width,
                entity.height,
            )
        })
        .collect();
    let relationships = diagram
        .relationships
        .iter()
        .enumerate()
        .map(|(index, rel)| {
            let points = routes.get(index).cloned().unwrap_or_default();
            let verb = (!rel.label.is_empty())
                .then(|| label_at(&points, &rel.label, &taken, &elsewhere(&routes, index)))
                .flatten();
            if let Some(placed) = verb {
                taken.push(placed);
            }
            PlacedRelationship {
                from: rel.from.clone(),
                to: rel.to.clone(),
                from_cardinality: rel.from_cardinality,
                to_cardinality: rel.to_cardinality,
                label: rel.label.clone(),
                identifying: rel.identifying,
                feet: feet(&points, rel.from_cardinality, rel.to_cardinality),
                label_at: verb.map(|placed| placed.at),
                points,
            }
        })
        .collect();

    Placed {
        width: placed.width,
        height: placed.height,
        entities,
        relationships,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::er::parse;

    fn sized(source: &str) -> layout::Node {
        let diagram = parse(source);
        measure(diagram.entities.first().expect("one entity"))
    }

    #[test]
    fn an_entity_with_no_columns_still_has_a_row_of_height() {
        let bare = sized("erDiagram\n  A ||--|| B : x");
        assert!((bare.width - MIN_WIDTH).abs() < 1e-9);
        assert!((bare.height - (HEADER_HEIGHT + ROW_HEIGHT)).abs() < 1e-9);
    }

    #[test]
    fn each_column_adds_a_row() {
        let one = sized("erDiagram\n  A {\n    int id PK\n  }");
        let two = sized("erDiagram\n  A {\n    int id PK\n    string name\n  }");
        assert!((two.height - one.height - ROW_HEIGHT).abs() < 1e-9);
    }

    #[test]
    fn a_box_widens_for_whatever_is_longest_in_it() {
        let short = sized("erDiagram\n  A ||--|| B : x");
        let long_name = sized("erDiagram\n  AN_ENTITY_WITH_A_VERY_LONG_NAME ||--|| B : x");
        assert!(long_name.width > short.width);
        let long_row =
            sized("erDiagram\n  A {\n    varchar a_rather_long_column_name_here UK\n  }");
        assert!(long_row.width > short.width);
    }

    #[test]
    fn a_badge_is_only_as_wide_as_it_needs_to_be() {
        let none = badge_width(&Attribute::default());
        assert!((none - 0.0).abs() < 1e-9);
        let one = attribute_with("PK");
        let two = attribute_with("PK FK");
        assert!(badge_width(&two) > badge_width(&one));
        assert!(badge_width(&one) > 0.0);
    }

    fn attribute_with(keys: &str) -> Attribute {
        crate::er::attribute(&format!("int id {keys}")).expect("an attribute")
    }

    #[test]
    fn exactly_one_is_two_bars_and_nothing_else() {
        let drawn = foot(
            Point::new(100.0, 0.0),
            Point::new(0.0, 0.0),
            Cardinality::One,
        );
        assert_eq!(drawn.bars.len(), 2);
        assert!(drawn.toes.is_empty());
        assert_eq!(drawn.ring, None);
        // Both bars sit back from the box, not on it.
        for (a, b) in &drawn.bars {
            assert!(a.x < 100.0 && b.x < 100.0);
            assert!((a.x - b.x).abs() < 1e-9, "a bar crosses the line");
        }
    }

    #[test]
    fn a_foot_fans_out_towards_the_box_it_touches() {
        let drawn = foot(
            Point::new(100.0, 0.0),
            Point::new(0.0, 0.0),
            Cardinality::ZeroMany,
        );
        assert_eq!(drawn.toes.len(), 3);
        assert!(drawn.bars.is_empty());
        let ring = drawn.ring.expect("a ring");
        // Every line of the foot ends at one point, further from the box.
        let converge = drawn.toes.first().expect("a line").1;
        for (_, end) in &drawn.toes {
            assert!((end.x - converge.x).abs() < 1e-9);
        }
        assert!(converge.x < 100.0 - FOOT_TIP);
        // And the ring sits past the point the foot converges at.
        assert!(ring.x <= converge.x);
    }

    #[test]
    fn one_or_more_is_a_foot_with_a_bar_behind_it() {
        let drawn = foot(
            Point::new(0.0, 100.0),
            Point::new(0.0, 0.0),
            Cardinality::Many,
        );
        assert_eq!(drawn.toes.len(), 3);
        assert_eq!(drawn.bars.len(), 1);
        assert_eq!(drawn.ring, None);
        // The bar is further from the box than the foot, so the two do not
        // overlap. Measuring downwards, further means a smaller y.
        let converge = drawn.toes.first().expect("a line").1;
        let (bar, _) = drawn.bars.first().expect("a bar");
        assert!(bar.y < converge.y);
    }

    #[test]
    fn one_or_none_is_a_bar_and_a_ring() {
        let drawn = foot(
            Point::new(100.0, 0.0),
            Point::new(0.0, 0.0),
            Cardinality::ZeroOne,
        );
        assert_eq!(drawn.bars.len(), 1);
        assert!(drawn.toes.is_empty());
        let ring = drawn.ring.expect("a ring");
        let (bar, _) = drawn.bars.first().expect("a bar");
        assert!(ring.x < bar.x, "the ring sits past the bar");
    }

    #[test]
    fn a_foot_turns_with_the_line_it_sits_on() {
        let rightwards = foot(
            Point::new(100.0, 0.0),
            Point::new(0.0, 0.0),
            Cardinality::One,
        );
        let downwards = foot(
            Point::new(0.0, 100.0),
            Point::new(0.0, 0.0),
            Cardinality::One,
        );
        let (a, b) = rightwards.bars.first().copied().expect("a bar");
        assert!(
            (a.x - b.x).abs() < 1e-9,
            "a bar across a level line is upright"
        );
        let (a, b) = downwards.bars.first().copied().expect("a bar");
        assert!(
            (a.y - b.y).abs() < 1e-9,
            "a bar across an upright line is level"
        );
    }

    #[test]
    fn a_foot_with_nowhere_to_point_is_empty_rather_than_a_division_by_nothing() {
        let nowhere = foot(
            Point::new(1.0, 1.0),
            Point::new(1.0, 1.0),
            Cardinality::Many,
        );
        assert_eq!(nowhere, Foot::default());
        assert_eq!(along(Point::new(1.0, 1.0), Point::new(1.0, 1.0)), None);
    }

    #[test]
    fn a_route_with_fewer_than_two_points_has_no_feet() {
        assert!(feet(&[], Cardinality::One, Cardinality::One).is_empty());
        assert!(feet(&[Point::new(0.0, 0.0)], Cardinality::One, Cardinality::One).is_empty());
        assert_eq!(
            feet(
                &[Point::new(0.0, 0.0), Point::new(10.0, 0.0)],
                Cardinality::One,
                Cardinality::One
            )
            .len(),
            2
        );
    }

    #[test]
    fn the_middle_of_a_route_is_measured_along_it() {
        // A straight run: the middle is the middle.
        assert_eq!(
            midpoint(&[Point::new(0.0, 0.0), Point::new(100.0, 0.0)]),
            Some((Point::new(50.0, 0.0), false, 0))
        );
        // An elbow: half the distance travelled, which is on the line rather
        // than in the corner it turns.
        let elbow = midpoint(&[
            Point::new(0.0, 0.0),
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
        ])
        .expect("a middle");
        assert!((elbow.0.x - 100.0).abs() < 1e-9);
        assert!((elbow.0.y - 0.0).abs() < 1e-9);
        // The mark lands where the run turns, which this reads as level — and
        // names the level leg, so the upright one past the corner is left free
        // to be treated as the obstacle it is.
        assert!(!elbow.1);
        assert_eq!(elbow.2, 0);
        // Nothing to walk along.
        assert_eq!(midpoint(&[]), None);
        assert_eq!(
            midpoint(&[Point::new(3.0, 4.0), Point::new(3.0, 4.0)]),
            Some((Point::new(3.0, 4.0), false, 0))
        );
    }

    #[test]
    fn a_verb_is_not_drawn_across_its_own_turn() {
        // The measured fault: an elbow puts the halfway mark on the corner, so
        // the label anchors to one leg with the other running straight through
        // where it wants to sit. `receives` and `generates` were both drawn with
        // their own line through the middle of the word.
        let placed = layout(&parse(
            "erDiagram\n  PRODUCT ||..o{ REVIEW : receives\n  PRODUCT ||--o{ ORDER : contains",
        ));
        for rel in &placed.relationships {
            let at = rel.label_at.expect("a verb");
            let (width, height) = label_size(&rel.label);
            let box_ = PlacedLabel::new(at, width, height);
            for pair in rel.points.windows(2) {
                let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
                    continue;
                };
                assert!(
                    !box_.crosses(*a, *b),
                    "{} sits on its own line {a:?}..{b:?}",
                    rel.label
                );
            }
        }
    }

    #[test]
    fn a_diagram_with_no_entities_lays_out_to_nothing() {
        assert_eq!(layout(&Diagram::default()), Placed::default());
    }

    #[test]
    fn every_entity_is_placed_and_every_relationship_routed() {
        let placed = layout(&parse("erDiagram\n  CUSTOMER ||--o{ ORDER : places"));
        assert_eq!(placed.entities.len(), 2);
        let rel = placed.relationships.first().expect("a line");
        assert!(rel.points.len() >= 2);
        assert_eq!(rel.feet.len(), 2);
        assert!(rel.label_at.is_some());
        // Laid out across the page, so the second box is to the right of the
        // first rather than under it.
        let (first, second) = (
            placed.entities.first().expect("a box"),
            placed.entities.get(1).expect("a box"),
        );
        assert!(second.at.x > first.at.x);
    }

    #[test]
    fn a_relationship_with_no_verb_has_nowhere_to_put_one() {
        let placed = layout(&parse("erDiagram\n  A ||--|| B"));
        assert_eq!(placed.relationships.first().expect("a line").label_at, None);
    }

    #[test]
    fn a_relationship_naming_an_entity_that_does_not_exist_is_dropped_not_drawn() {
        let diagram = Diagram {
            entities: vec![Entity {
                id: "A".into(),
                label: "A".into(),
                attributes: Vec::new(),
            }],
            relationships: vec![super::super::types::Relationship {
                from: "A".into(),
                to: "Nowhere".into(),
                from_cardinality: Cardinality::One,
                to_cardinality: Cardinality::One,
                label: "x".into(),
                identifying: true,
            }],
        };
        let placed = layout(&diagram);
        let rel = placed.relationships.first().expect("a line");
        assert!(rel.points.is_empty());
        assert!(rel.feet.is_empty());
    }
}
