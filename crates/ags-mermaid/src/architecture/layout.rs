//! Sizing the boxes and arranging them on the grid the side letters describe.
//!
//! A group is laid out from its own children first and then placed as one box
//! among its siblings, so nothing ever ends up inside a frame it does not
//! belong to — the defect the flowchart's subgraphs still have, avoided here by
//! never putting two containers' children in one grid.

use crate::metrics::text_width;
use crate::scene::Point;

use super::grid::{place, Cell, Link};
use super::types::{Diagram, Edge, Kind};

/// Around the whole drawing.
pub const PADDING: f64 = 32.0;
/// Inside a group, round what it holds.
pub const GROUP_PAD: f64 = 18.0;
/// The band at the top of a group that holds its name.
pub const HEADER: f64 = 26.0;
pub const SERVICE_MIN_WIDTH: f64 = 92.0;
pub const SERVICE_HEIGHT: f64 = 84.0;
/// The glyph inside a service.
pub const ICON: f64 = 38.0;
/// Above the glyph, and between it and the name under it.
pub const ICON_TOP: f64 = 12.0;
pub const ICON_GAP: f64 = 8.0;
pub const JUNCTION: f64 = 18.0;
pub const LABEL_FONT: f64 = 12.0;
pub const LABEL_WEIGHT: u32 = 500;
pub const TITLE_FONT: f64 = 12.0;
pub const TITLE_WEIGHT: u32 = 600;
/// Between one thing and the next, either way.
pub const GAP: f64 = 40.0;
/// Either side of a service's name, inside its box.
const LABEL_PAD: f64 = 8.0;

/// One thing, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedItem {
    pub id: String,
    pub kind: Kind,
    pub icon: String,
    pub title: String,
    pub at: Point,
    pub width: f64,
    pub height: f64,
    /// How many groups enclose this. Outermost is nought.
    pub depth: usize,
}

impl PlacedItem {
    pub fn centre(&self) -> Point {
        Point::new(self.at.x + self.width / 2.0, self.at.y + self.height / 2.0)
    }
}

/// One line, routed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedEdge {
    pub from: String,
    pub to: String,
    pub arrow_start: bool,
    pub arrow_end: bool,
    pub points: Vec<Point>,
}

/// A laid-out architecture diagram.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub items: Vec<PlacedItem>,
    pub edges: Vec<PlacedEdge>,
}

/// How big a service box has to be to hold its name.
pub fn service_size(title: &str) -> (f64, f64) {
    let label = text_width(title, LABEL_FONT, LABEL_WEIGHT);
    (
        SERVICE_MIN_WIDTH.max(label.ceil() + LABEL_PAD * 2.0),
        SERVICE_HEIGHT,
    )
}

/// The group `at` sits in, when it names one that exists.
fn parent_of(diagram: &Diagram, at: usize) -> Option<usize> {
    let item = diagram.items.get(at)?;
    let parent = diagram.index_of(&item.parent)?;
    (diagram.items.get(parent)?.kind == Kind::Group).then_some(parent)
}

/// Everything directly inside `container`, in declaration order.
fn children_of(diagram: &Diagram, container: Option<usize>) -> Vec<usize> {
    (0..diagram.items.len())
        .filter(|at| parent_of(diagram, *at) == container)
        .collect()
}

/// The thing directly inside `container` that holds `at`, or `at` itself.
///
/// An edge may join two services nested at different depths; the grid it
/// constrains is the one whose squares they each sit under.
fn under(diagram: &Diagram, at: usize, container: Option<usize>) -> Option<usize> {
    let mut here = at;
    // A group cannot hold itself, so walking up terminates; the bound is there
    // because the diagram is built from a source and a source can say anything.
    for _ in 0..=diagram.items.len() {
        if parent_of(diagram, here) == container {
            return Some(here);
        }
        here = parent_of(diagram, here)?;
    }
    None
}

/// Where the far end of an edge sits, in whole cells.
///
/// The side an edge leaves says which way the other end lies. When the two
/// sides do not face each other — `j:L -- T:api1` — both say something, and the
/// two claims add: api1 is to the left of j, and below it, because its top is
/// what faces j.
pub fn step_of(edge: &Edge) -> (i64, i64) {
    match (edge.from_side, edge.to_side) {
        (Some(from), Some(to)) => {
            let (dx, dy) = from.step();
            if to == from.opposite() {
                return (dx, dy);
            }
            let (tx, ty) = to.step();
            (dx - tx, dy - ty)
        }
        (Some(from), None) => from.step(),
        (None, Some(to)) => {
            let (tx, ty) = to.step();
            (-tx, -ty)
        }
        (None, None) => (1, 0),
    }
}

/// The constraints among one container's children.
fn links(diagram: &Diagram, kids: &[usize], container: Option<usize>) -> Vec<Link> {
    let slot = |id: &str| {
        let at = diagram.index_of(id)?;
        let held = under(diagram, at, container)?;
        kids.iter().position(|kid| *kid == held)
    };
    diagram
        .edges
        .iter()
        .filter_map(|edge| {
            let (from, to) = (slot(&edge.from)?, slot(&edge.to)?);
            (from != to).then(|| Link {
                from,
                to,
                step: step_of(edge),
            })
        })
        .collect()
}

/// The width of each column and the height of each row of a grid.
fn tracks(cells: &[Cell], sizes: &[(f64, f64)]) -> (Vec<f64>, Vec<f64>) {
    let extent = |pick: fn(&Cell) -> i64| {
        cells
            .iter()
            .map(pick)
            .max()
            .map_or(0, |most| most + 1)
            .max(0)
    };
    let columns = usize::try_from(extent(|cell| cell.col)).unwrap_or(0);
    let rows = usize::try_from(extent(|cell| cell.row)).unwrap_or(0);
    let mut widths = vec![0.0_f64; columns];
    let mut heights = vec![0.0_f64; rows];
    for (cell, (width, height)) in cells.iter().zip(sizes) {
        if let Some(slot) = usize::try_from(cell.col)
            .ok()
            .and_then(|c| widths.get_mut(c))
        {
            *slot = slot.max(*width);
        }
        if let Some(slot) = usize::try_from(cell.row)
            .ok()
            .and_then(|r| heights.get_mut(r))
        {
            *slot = slot.max(*height);
        }
    }
    (widths, heights)
}

/// Where each track starts, and how long the whole run is.
fn offsets(tracks: &[f64]) -> (Vec<f64>, f64) {
    let mut starts = Vec::with_capacity(tracks.len());
    let mut at = 0.0;
    for length in tracks {
        starts.push(at);
        at += length + GAP;
    }
    (starts, (at - GAP).max(0.0))
}

/// The state an arrangement fills in: every thing's size, and where it sits
/// inside whatever holds it.
struct Arrangement {
    sizes: Vec<(f64, f64)>,
    local: Vec<Point>,
}

/// Lay out one container's children and report how big the container's content
/// came out.
///
/// Recursive: a group's own children are arranged first, which is what gives it
/// the size it is placed at among its siblings.
fn arrange(
    diagram: &Diagram,
    container: Option<usize>,
    depth: usize,
    out: &mut Arrangement,
) -> (f64, f64) {
    let kids = children_of(diagram, container);
    if kids.is_empty() || depth > diagram.items.len() {
        return (0.0, 0.0);
    }
    for kid in &kids {
        let size = match diagram.items.get(*kid).map(|item| item.kind) {
            Some(Kind::Group) => {
                let (width, height) = arrange(diagram, Some(*kid), depth + 1, out);
                (width + GROUP_PAD * 2.0, height + GROUP_PAD * 2.0 + HEADER)
            }
            Some(Kind::Junction) => (JUNCTION, JUNCTION),
            _ => diagram
                .items
                .get(*kid)
                .map_or((SERVICE_MIN_WIDTH, SERVICE_HEIGHT), |item| {
                    service_size(&item.title)
                }),
        };
        if let Some(slot) = out.sizes.get_mut(*kid) {
            *slot = size;
        }
    }
    let sizes: Vec<(f64, f64)> = kids
        .iter()
        .map(|kid| out.sizes.get(*kid).copied().unwrap_or_default())
        .collect();
    let cells = place(kids.len(), &links(diagram, &kids, container));
    let (widths, heights) = tracks(&cells, &sizes);
    let (lefts, total_width) = offsets(&widths);
    let (tops, total_height) = offsets(&heights);
    for ((kid, cell), (width, height)) in kids.iter().zip(&cells).zip(&sizes) {
        let column = usize::try_from(cell.col).unwrap_or(0);
        let row = usize::try_from(cell.row).unwrap_or(0);
        let track_w = widths.get(column).copied().unwrap_or(*width);
        let track_h = heights.get(row).copied().unwrap_or(*height);
        // Centred in its square, so a short box in a tall row does not look
        // like it has slipped.
        let at = Point::new(
            lefts.get(column).copied().unwrap_or(0.0) + (track_w - width) / 2.0,
            tops.get(row).copied().unwrap_or(0.0) + (track_h - height) / 2.0,
        );
        if let Some(slot) = out.local.get_mut(*kid) {
            *slot = at;
        }
    }
    (total_width, total_height)
}

/// Turn positions relative to a container into positions on the canvas.
fn absolute(diagram: &Diagram, arrangement: &Arrangement) -> Vec<Point> {
    let mut out = vec![Point::default(); diagram.items.len()];
    for at in 0..diagram.items.len() {
        let mut point = arrangement.local.get(at).copied().unwrap_or_default();
        let mut here = at;
        // Bounded for the same reason `under` is.
        for _ in 0..=diagram.items.len() {
            let Some(parent) = parent_of(diagram, here) else {
                break;
            };
            let origin = arrangement.local.get(parent).copied().unwrap_or_default();
            point = Point::new(
                point.x + origin.x + GROUP_PAD,
                point.y + origin.y + GROUP_PAD + HEADER,
            );
            here = parent;
        }
        if let Some(slot) = out.get_mut(at) {
            *slot = Point::new(point.x + PADDING, point.y + PADDING);
        }
    }
    out
}

/// How many groups enclose a thing.
fn depth_of(diagram: &Diagram, at: usize) -> usize {
    let mut depth = 0;
    let mut here = at;
    while let Some(parent) = parent_of(diagram, here) {
        depth += 1;
        here = parent;
        if depth > diagram.items.len() {
            break;
        }
    }
    depth
}

/// Place every declared thing. Routing is left to [`super::route`].
pub fn boxes(diagram: &Diagram) -> Vec<PlacedItem> {
    let mut arrangement = Arrangement {
        sizes: vec![(0.0, 0.0); diagram.items.len()],
        local: vec![Point::default(); diagram.items.len()],
    };
    arrange(diagram, None, 0, &mut arrangement);
    let at = absolute(diagram, &arrangement);
    diagram
        .items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let (width, height) = arrangement.sizes.get(index).copied().unwrap_or_default();
            PlacedItem {
                id: item.id.clone(),
                kind: item.kind,
                icon: item.icon.clone(),
                title: item.title.clone(),
                at: at.get(index).copied().unwrap_or_default(),
                width,
                height,
                depth: depth_of(diagram, index),
            }
        })
        .collect()
}

/// Lay out a parsed architecture diagram.
pub fn layout(diagram: &Diagram) -> Placed {
    if diagram.items.is_empty() {
        return Placed::default();
    }
    let items = boxes(diagram);
    let edges = super::route::routes(diagram, &items);
    let mut width = 0.0_f64;
    let mut height = 0.0_f64;
    for item in &items {
        width = width.max(item.at.x + item.width + PADDING);
        height = height.max(item.at.y + item.height + PADDING);
    }
    for edge in &edges {
        for point in &edge.points {
            width = width.max(point.x + PADDING);
            height = height.max(point.y + PADDING);
        }
    }
    Placed {
        width,
        height,
        items,
        edges,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::architecture::parse;
    use crate::architecture::types::Side;

    fn placed(source: &str) -> Vec<PlacedItem> {
        boxes(&parse(source))
    }

    fn find<'a>(items: &'a [PlacedItem], id: &str) -> &'a PlacedItem {
        items
            .iter()
            .find(|item| item.id == id)
            .unwrap_or_else(|| panic!("no {id}"))
    }

    fn edge(from: Option<Side>, to: Option<Side>) -> Edge {
        Edge {
            from: "a".into(),
            from_side: from,
            to: "b".into(),
            to_side: to,
            arrow_start: false,
            arrow_end: false,
        }
    }

    #[test]
    fn a_service_box_is_wide_enough_for_its_name() {
        let (narrow, height) = service_size("Web");
        assert!((narrow - SERVICE_MIN_WIDTH).abs() < 1e-9);
        assert!((height - SERVICE_HEIGHT).abs() < 1e-9);
        let (wide, _) = service_size("A Service With A Very Long Name");
        assert!(wide > narrow);
    }

    #[test]
    fn two_sides_that_face_each_other_say_one_thing() {
        assert_eq!(step_of(&edge(Some(Side::Right), Some(Side::Left))), (1, 0));
        assert_eq!(step_of(&edge(Some(Side::Bottom), Some(Side::Top))), (0, 1));
        assert_eq!(step_of(&edge(Some(Side::Left), Some(Side::Right))), (-1, 0));
        assert_eq!(step_of(&edge(Some(Side::Top), Some(Side::Bottom))), (0, -1));
    }

    #[test]
    fn two_sides_that_do_not_face_each_other_say_two() {
        // `j:L -- T:api1` — to the left, and below, because its top faces j.
        assert_eq!(step_of(&edge(Some(Side::Left), Some(Side::Top))), (-1, 1));
        assert_eq!(step_of(&edge(Some(Side::Right), Some(Side::Top))), (1, 1));
        assert_eq!(
            step_of(&edge(Some(Side::Right), Some(Side::Bottom))),
            (1, -1)
        );
    }

    #[test]
    fn one_side_alone_is_enough_and_none_at_all_reads_left_to_right() {
        assert_eq!(step_of(&edge(Some(Side::Bottom), None)), (0, 1));
        // Only the far side written: it faces back the way it came.
        assert_eq!(step_of(&edge(None, Some(Side::Left))), (1, 0));
        assert_eq!(step_of(&edge(None, Some(Side::Top))), (0, 1));
        assert_eq!(step_of(&edge(None, None)), (1, 0));
    }

    #[test]
    fn a_track_is_as_long_as_the_longest_thing_in_it() {
        let cells = [Cell::new(0, 0), Cell::new(0, 1), Cell::new(1, 0)];
        let sizes = [(50.0, 10.0), (90.0, 20.0), (30.0, 40.0)];
        let (widths, heights) = tracks(&cells, &sizes);
        assert_eq!(widths, [90.0, 30.0]);
        assert_eq!(heights, [40.0, 20.0]);
    }

    #[test]
    fn an_empty_grid_has_no_tracks() {
        let (widths, heights) = tracks(&[], &[]);
        assert!(widths.is_empty() && heights.is_empty());
    }

    #[test]
    fn tracks_are_laid_end_to_end_with_a_gap_between() {
        let (starts, total) = offsets(&[100.0, 50.0]);
        assert_eq!(starts, [0.0, 100.0 + GAP]);
        assert!((total - (150.0 + GAP)).abs() < 1e-9);
        let (none, empty) = offsets(&[]);
        assert!(none.is_empty());
        assert!((empty - 0.0).abs() < 1e-9);
    }

    #[test]
    fn a_side_letter_decides_where_a_thing_goes() {
        let items = placed(
            "architecture-beta\n  service web(server)[Web]\n  service db(database)[DB]\n  web:R -- L:db",
        );
        let (web, db) = (find(&items, "web"), find(&items, "db"));
        assert!(db.at.x > web.at.x, "db is to the right of web");
        assert!((db.at.y - web.at.y).abs() < 1e-9, "and level with it");
    }

    #[test]
    fn a_thing_named_below_another_goes_below_it() {
        let items = placed(
            "architecture-beta\n  service db(database)[DB]\n  service disk(disk)[Storage]\n  db:B -- T:disk",
        );
        let (db, disk) = (find(&items, "db"), find(&items, "disk"));
        assert!(disk.at.y > db.at.y);
        assert!((disk.at.x - db.at.x).abs() < 1e-9);
    }

    #[test]
    fn a_group_is_drawn_round_what_it_holds() {
        let items = placed(
            "architecture-beta\n  group cloud(cloud)[Cloud]\n  service web(server)[Web] in cloud\n  service db(database)[DB] in cloud\n  web:R -- L:db",
        );
        let cloud = find(&items, "cloud");
        for id in ["web", "db"] {
            let held = find(&items, id);
            assert!(held.at.x >= cloud.at.x, "{id} reaches past the left");
            assert!(held.at.y >= cloud.at.y + HEADER, "{id} sits in the header");
            assert!(held.at.x + held.width <= cloud.at.x + cloud.width);
            assert!(held.at.y + held.height <= cloud.at.y + cloud.height);
        }
        assert_eq!(cloud.depth, 0);
        assert_eq!(find(&items, "web").depth, 1);
    }

    #[test]
    fn a_nested_group_sits_inside_the_one_that_holds_it() {
        let items = placed(
            "architecture-beta\n  group cloud(cloud)[Cloud]\n  group region(server)[Region A] in cloud\n  service web(server)[Web] in region\n  service cdn(internet)[CDN] in cloud\n  cdn:L --> T:web",
        );
        let (cloud, region, web) = (
            find(&items, "cloud"),
            find(&items, "region"),
            find(&items, "web"),
        );
        assert!(region.at.x >= cloud.at.x && region.at.y >= cloud.at.y);
        assert!(region.at.x + region.width <= cloud.at.x + cloud.width);
        assert!(region.at.y + region.height <= cloud.at.y + cloud.height);
        assert!(web.at.x >= region.at.x && web.at.y >= region.at.y + HEADER);
        assert_eq!(region.depth, 1);
        assert_eq!(web.depth, 2);
    }

    #[test]
    fn nothing_ends_up_inside_a_frame_it_does_not_belong_to() {
        let items = placed(
            "architecture-beta\n  group left(cloud)[Left]\n  group right(cloud)[Right]\n  service a(server)[A] in left\n  service b(server)[B] in right\n  a:R --> L:b",
        );
        let (left, right) = (find(&items, "left"), find(&items, "right"));
        let inside = |frame: &PlacedItem, held: &PlacedItem| {
            held.at.x < frame.at.x + frame.width
                && held.at.x + held.width > frame.at.x
                && held.at.y < frame.at.y + frame.height
                && held.at.y + held.height > frame.at.y
        };
        assert!(!inside(left, find(&items, "b")));
        assert!(!inside(right, find(&items, "a")));
        // And the two frames do not overlap each other either.
        assert!(!inside(left, right));
    }

    #[test]
    fn nothing_is_placed_at_a_negative_coordinate() {
        let items = placed(
            "architecture-beta\n  service j(server)[J]\n  service a(server)[A]\n  service b(server)[B]\n  j:L -- T:a\n  j:R -- T:b",
        );
        for item in &items {
            assert!(item.at.x >= 0.0 && item.at.y >= 0.0, "{}", item.id);
        }
    }

    #[test]
    fn a_diagram_with_nothing_in_it_lays_out_to_nothing() {
        assert_eq!(layout(&Diagram::default()), Placed::default());
    }

    #[test]
    fn the_canvas_holds_everything_drawn_on_it() {
        let drawn = layout(&parse(
            "architecture-beta\n  group cloud(cloud)[Cloud]\n  service web(server)[Web] in cloud\n  service db(database)[DB] in cloud\n  web:R -- L:db",
        ));
        for item in &drawn.items {
            assert!(item.at.x + item.width <= drawn.width, "{}", item.id);
            assert!(item.at.y + item.height <= drawn.height, "{}", item.id);
        }
        for edge in &drawn.edges {
            for point in &edge.points {
                assert!(point.x <= drawn.width && point.y <= drawn.height);
            }
        }
    }

    #[test]
    fn a_group_that_names_itself_does_not_loop_forever() {
        // Nothing produces this from a real source; the reader will take it
        // from one, so the walk up the tree is bounded.
        let diagram =
            parse("architecture-beta\n  group a(cloud)[A] in a\n  service s(server)[S] in a");
        let items = boxes(&diagram);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn a_thing_held_by_a_group_that_holds_itself_is_under_nothing() {
        // Not reachable from a well-formed source, but a source can say
        // anything, and the walk up the tree has to end.
        let diagram =
            parse("architecture-beta\n  group a(cloud)[A] in a\n  service s(server)[S] in a");
        assert_eq!(under(&diagram, 1, None), None);
        assert_eq!(under(&diagram, 0, None), None);
        // And a thing at the top is directly under it.
        let plain = parse("architecture-beta\n  service s(server)[S]");
        assert_eq!(under(&plain, 0, None), Some(0));
    }

    #[test]
    fn a_thing_that_names_a_group_nobody_declared_sits_at_the_top() {
        let items = placed("architecture-beta\n  service web(server)[Web] in nowhere");
        assert_eq!(find(&items, "web").depth, 0);
    }

    #[test]
    fn the_same_source_twice_gives_the_same_placement() {
        let source = "architecture-beta\n  group cloud(cloud)[Cloud]\n  service web(server)[Web] in cloud\n  service db(database)[DB] in cloud\n  service disk(disk)[S] in cloud\n  web:R -- L:db\n  db:B -- T:disk";
        assert_eq!(layout(&parse(source)), layout(&parse(source)));
    }
}
