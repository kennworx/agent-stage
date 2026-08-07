//! Where each box goes.
//!
//! Children are packed into their parent by the squarified algorithm (Bruls,
//! Huizing and van Wijk): rows are grown while the worst aspect ratio in them
//! keeps improving, which keeps cells close to square and so readable. Areas map
//! straight from value, so the picture is honest about proportion.
//!
//! Cells come out parents-first, so a container is drawn behind its contents.

use crate::scene::Point;

use super::types::{Node, Treemap};

pub const WIDTH: f64 = 600.0;
pub const HEIGHT: f64 = 400.0;
pub const PADDING: f64 = 16.0;
pub const TITLE_HEIGHT: f64 = 40.0;
pub const TITLE_FONT: f64 = 18.0;
/// Reserved at the top of a branch for its name.
pub const HEADER_HEIGHT: f64 = 20.0;
/// Between a branch's inner area and what it holds.
pub const CELL_PAD: f64 = 3.0;

/// A rectangle, in screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub at: Point,
    pub width: f64,
    pub height: f64,
}

/// One box, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub path: String,
    pub label: String,
    pub value: f64,
    pub rect: Rect,
    pub depth: usize,
    pub is_leaf: bool,
    pub color_index: Option<usize>,
}

/// A laid-out treemap.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<(String, Point)>,
    /// Parents before children, so containers paint behind their contents.
    pub cells: Vec<Cell>,
}

/// Where the diagram's name sits: the middle of the band reserved above it.
fn title_baseline() -> f64 {
    f64::midpoint(PADDING, TITLE_HEIGHT)
}

/// One item waiting to be packed: a node and the area it is owed.
struct Item<'a> {
    node: &'a Node,
    area: f64,
}

/// The worst aspect ratio in a candidate row laid along `length`.
///
/// This is what the algorithm minimises: a row is grown while adding to it
/// makes the worst cell in it *less* elongated, and stopped when it would not.
fn worst_ratio(row: &[&Item<'_>], length: f64, sum: f64) -> f64 {
    if sum <= 0.0 || length <= 0.0 {
        return f64::INFINITY;
    }
    let max = row.iter().map(|i| i.area).fold(f64::NEG_INFINITY, f64::max);
    let min = row.iter().map(|i| i.area).fold(f64::INFINITY, f64::min);
    let s2 = sum * sum;
    let l2 = length * length;
    (l2 * max / s2).max(s2 / (l2 * min))
}

/// Pack `items` into `rect`, calling `assign` with the rectangle each lands in.
fn squarify<'a>(items: &[Item<'a>], rect: Rect, assign: &mut impl FnMut(&'a Node, Rect)) {
    let mut remaining: Vec<&Item<'a>> = items.iter().filter(|i| i.area > 0.0).collect();
    let (mut x, mut y) = (rect.at.x, rect.at.y);
    let (mut w, mut h) = (rect.width, rect.height);

    while !remaining.is_empty() && w > 0.0 && h > 0.0 {
        let shortest = w.min(h);
        let mut row: Vec<&Item<'a>> = Vec::new();
        let mut row_area = 0.0;

        while let Some(next) = remaining.first().copied() {
            let grown_area = row_area + next.area;
            let mut grown = row.clone();
            grown.push(next);
            if row.is_empty()
                || worst_ratio(&grown, shortest, grown_area)
                    <= worst_ratio(&row, shortest, row_area)
            {
                row.push(next);
                row_area = grown_area;
                remaining.remove(0);
            } else {
                break;
            }
        }

        // The row runs along whichever side is shorter, so its cells stay as
        // close to square as the values allow.
        if w >= h {
            let row_w = row_area / h;
            let mut cy = y;
            for item in &row {
                let cell_h = item.area / row_area * h;
                assign(
                    item.node,
                    Rect {
                        at: Point::new(x, cy),
                        width: row_w,
                        height: cell_h,
                    },
                );
                cy += cell_h;
            }
            x += row_w;
            w -= row_w;
        } else {
            let row_h = row_area / w;
            let mut cx = x;
            for item in &row {
                let cell_w = item.area / row_area * w;
                assign(
                    item.node,
                    Rect {
                        at: Point::new(cx, y),
                        width: cell_w,
                        height: row_h,
                    },
                );
                cx += cell_w;
            }
            y += row_h;
            h -= row_h;
        }
    }
}

/// Scale each child's value to an area filling `rect`, then pack them.
fn pack<'a>(children: &'a [Node], rect: Rect, assign: &mut impl FnMut(&'a Node, Rect)) {
    let total: f64 = children.iter().map(|c| c.value.max(0.0)).sum();
    if total <= 0.0 || rect.width <= 0.0 || rect.height <= 0.0 {
        return;
    }
    let scale = rect.width * rect.height / total;
    let items: Vec<Item<'a>> = children
        .iter()
        .map(|node| Item {
            node,
            area: node.value.max(0.0) * scale,
        })
        .collect();
    squarify(&items, rect, assign);
}

/// Place one node and everything inside it.
fn place(node: &Node, rect: Rect, depth: usize, out: &mut Vec<Cell>) {
    out.push(Cell {
        path: node.path.clone(),
        label: node.label.clone(),
        value: node.value,
        rect,
        depth,
        is_leaf: node.is_leaf(),
        color_index: node.color_index,
    });
    if node.is_leaf() {
        return;
    }
    // A branch too short to show a name does not reserve room for one.
    let header = if rect.height > HEADER_HEIGHT * 2.0 {
        HEADER_HEIGHT
    } else {
        0.0
    };
    let inner = Rect {
        at: Point::new(rect.at.x + CELL_PAD, rect.at.y + header),
        width: rect.width - CELL_PAD * 2.0,
        height: rect.height - header - CELL_PAD,
    };
    // Below a couple of pixels there is nothing left to divide.
    if inner.width <= 2.0 || inner.height <= 2.0 {
        return;
    }
    pack(&node.children, inner, &mut |child, r| {
        place(child, r, depth + 1, out);
    });
}

/// Lay out a parsed treemap.
pub fn layout(tree: &Treemap) -> Placed {
    let top = PADDING
        + if tree.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        };
    let content = Rect {
        at: Point::new(PADDING, top),
        width: WIDTH - PADDING * 2.0,
        height: HEIGHT - top - PADDING,
    };
    let mut cells = Vec::new();
    if tree.root.label.is_empty() {
        // A nameless container is not drawn; its children fill the canvas.
        pack(&tree.root.children, content, &mut |child, r| {
            place(child, r, 0, &mut cells);
        });
    } else {
        place(&tree.root, content, 0, &mut cells);
    }
    Placed {
        // The canvas is fixed: a treemap is about proportion, and letting it
        // resize with the data would make two of them incomparable.
        width: WIDTH,
        height: HEIGHT,
        title: tree
            .title
            .clone()
            .map(|text| (text, Point::new(WIDTH / 2.0, title_baseline()))),
        cells,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::treemap::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const TREE: &str = "treemap-beta\n\
        title Disk\n\
        \"Projects\"\n    \
            \"rust\" : 40\n    \
            \"web\"\n        \
                \"src\" : 20\n        \
                \"dist\" : 5";

    #[test]
    fn the_canvas_is_the_same_whatever_the_data() {
        for source in ["treemap\n\"a\" : 1", TREE] {
            let out = placed(source);
            assert!((out.width - WIDTH).abs() < 1e-9, "{source}");
            assert!((out.height - HEIGHT).abs() < 1e-9, "{source}");
        }
    }

    #[test]
    fn a_parent_comes_before_its_children() {
        let out = placed(TREE);
        let paths: Vec<&str> = out.cells.iter().map(|c| c.path.as_str()).collect();
        let parent = paths
            .iter()
            .position(|p| *p == "Projects/web")
            .expect("web");
        let child = paths
            .iter()
            .position(|p| *p == "Projects/web/src")
            .expect("src");
        assert!(parent < child);
    }

    #[test]
    fn area_follows_value() {
        let out = placed("treemap\n\"top\"\n  \"big\" : 3\n  \"small\" : 1");
        let area = |path: &str| {
            out.cells
                .iter()
                .find(|c| c.path == path)
                .map(|c| c.rect.width * c.rect.height)
                .expect(path)
        };
        let ratio = area("top/big") / area("top/small");
        assert!((ratio - 3.0).abs() < 0.01, "{ratio}");
    }

    #[test]
    fn every_cell_stays_inside_its_parent() {
        let out = placed(TREE);
        for cell in &out.cells {
            assert!(cell.rect.at.x >= PADDING - 1e-6, "{cell:?}");
            assert!(
                cell.rect.at.x + cell.rect.width <= WIDTH - PADDING + 1e-6,
                "{cell:?}"
            );
            assert!(
                cell.rect.at.y + cell.rect.height <= HEIGHT - PADDING + 1e-6,
                "{cell:?}"
            );
        }
    }

    #[test]
    fn siblings_do_not_overlap() {
        let out = placed("treemap\n\"top\"\n  \"a\" : 3\n  \"b\" : 2\n  \"c\" : 1");
        let cells: Vec<&Cell> = out.cells.iter().filter(|c| c.depth == 1).collect();
        for (i, one) in cells.iter().enumerate() {
            for two in cells.iter().skip(i + 1) {
                let apart = one.rect.at.x + one.rect.width <= two.rect.at.x + 1e-6
                    || two.rect.at.x + two.rect.width <= one.rect.at.x + 1e-6
                    || one.rect.at.y + one.rect.height <= two.rect.at.y + 1e-6
                    || two.rect.at.y + two.rect.height <= one.rect.at.y + 1e-6;
                assert!(apart, "{one:?} overlaps {two:?}");
            }
        }
    }

    #[test]
    fn a_branch_reserves_a_header_only_when_it_is_tall_enough() {
        let out = placed(TREE);
        let web = out
            .cells
            .iter()
            .find(|c| c.path == "Projects/web")
            .expect("web");
        let src = out
            .cells
            .iter()
            .find(|c| c.path == "Projects/web/src")
            .expect("src");
        if web.rect.height > HEADER_HEIGHT * 2.0 {
            assert!(src.rect.at.y >= web.rect.at.y + HEADER_HEIGHT - 1e-6);
        }
    }

    #[test]
    fn a_nameless_container_is_not_drawn() {
        let out = placed("treemap\n\"a\" : 1\n\"b\" : 2");
        assert_eq!(out.cells.len(), 2);
        assert!(out.cells.iter().all(|c| !c.label.is_empty()));
    }

    #[test]
    fn a_value_of_nothing_places_nothing() {
        // Zero area is not a cell; dividing by the total would be a division
        // by zero if every value were zero.
        let out = placed("treemap\n\"top\"\n  \"a\" : 0\n  \"b\" : 0");
        assert_eq!(out.cells.len(), 1, "only the root");
    }

    #[test]
    fn the_squarifier_prefers_squares_to_slivers() {
        // Four equal values in a wide rectangle should come out roughly square
        // rather than as four full-height columns.
        let out = placed("treemap\n\"top\"\n  \"a\" : 1\n  \"b\" : 1\n  \"c\" : 1\n  \"d\" : 1");
        for cell in out.cells.iter().filter(|c| c.depth == 1) {
            let ratio =
                (cell.rect.width / cell.rect.height).max(cell.rect.height / cell.rect.width);
            assert!(ratio < 2.0, "{cell:?} is a sliver");
        }
    }

    #[test]
    fn a_title_takes_room_from_the_content_not_from_the_canvas() {
        let bare = placed("treemap\n\"a\" : 1");
        let titled = placed("treemap\ntitle T\n\"a\" : 1");
        assert!((bare.height - titled.height).abs() < 1e-9);
        assert!(titled.cells[0].rect.height < bare.cells[0].rect.height);
        let (_, at) = titled.title.clone().expect("a title");
        assert!((at.x - WIDTH / 2.0).abs() < 1e-9);
    }
}
