//! Where the boxes and their boundary frames go.
//!
//! Elements are placed **by group**, not in one flat declaration-order grid.
//! That is what makes a boundary's frame honest: because a group's members are
//! laid out contiguously and the frame is drawn around that block, a non-member
//! can never fall inside it. A flat grid plus a bounding-box frame lets any
//! element that happens to sit between two members be visually captured by a
//! boundary it does not belong to — which inverts the meaning of the diagram.

use std::collections::HashSet;

use crate::text::wrap;

use super::config as l;
use super::geom::{count, Rect};
use super::pack::{pack_rows, Size};
use super::positioned::{PlacedBoundary, PlacedElement};
use super::types::{Boundary, Diagram, Element, ElementKind};

/// Human-readable kind tag shown above an element's label.
///
/// The technology is deliberately *not* folded in: the renderer already draws it
/// on its own line beneath the label, so embedding it here printed it twice and
/// made every box wide enough for the longer of two copies of it.
pub fn kind_tag(el: &Element) -> &'static str {
    match (el.kind, el.external) {
        (ElementKind::Person, false) => "«Person»",
        (ElementKind::Person, true) => "«External Person»",
        (ElementKind::System, false) => "«System»",
        (ElementKind::System, true) => "«External System»",
        (ElementKind::Container, _) => "«Container»",
        (ElementKind::Component, _) => "«Component»",
    }
}

/// One element with its derived tag and wrapped description.
#[derive(Debug, Clone, PartialEq)]
pub struct Shell {
    /// Index into the diagram's element list.
    pub index: usize,
    pub tag: &'static str,
    pub descr: Vec<String>,
}

/// The uniform box size, and the shells sized against it.
///
/// One size for every box: a grid of ragged boxes has no columns for edges to be
/// routed between.
pub fn size_boxes(elements: &[Element]) -> (Size, Vec<Shell>) {
    // Width is driven by the tag, label and technology — not the wrappable
    // description, which reflows to whatever width they settle on.
    let mut box_w = l::BOX_MIN_W;
    for el in elements {
        let label_w = crate::metrics::text_width(&el.label, l::LABEL_FONT, l::LABEL_WEIGHT);
        let tag_w = crate::metrics::text_width(kind_tag(el), l::TAG_FONT, l::TAG_WEIGHT);
        let techn_w = el.techn.as_ref().map_or(0.0, |t| {
            crate::metrics::text_width(t, l::TECHN_FONT, l::TECHN_WEIGHT)
        });
        box_w = box_w.max(label_w.max(tag_w).max(techn_w) + 2.0 * l::INNER_PAD_X);
    }
    box_w = box_w.min(l::BOX_MAX_W);
    let inner_w = box_w - 2.0 * l::INNER_PAD_X;

    let shells: Vec<Shell> = elements
        .iter()
        .enumerate()
        .map(|(index, el)| Shell {
            index,
            tag: kind_tag(el),
            descr: el.descr.as_ref().map_or_else(Vec::new, |d| {
                wrap(
                    d,
                    inner_w,
                    l::DESCR_FONT,
                    l::DESCR_WEIGHT,
                    l::DESCR_MAX_LINES,
                )
            }),
        })
        .collect();

    // Vertical room reserved for the icon strip above the tag.
    let icon_band = l::ICON_SIZE + l::ICON_GAP;
    let base = l::TOP_PAD + icon_band + l::TAG_H + l::LABEL_H + l::BOT_PAD;
    let mut box_h = base;
    for s in &shells {
        let techn = elements
            .get(s.index)
            .and_then(|e| e.techn.as_ref())
            .map_or(0.0, |_| l::TECHN_H);
        box_h = box_h.max(base + techn + count(s.descr.len()) * l::DESCR_H);
    }

    (Size::new(box_w, box_h), shells)
}

/// How deeply boundaries nest below `alias`.
///
/// Walks with one shared visited set, so a boundary reached down one branch is
/// not counted again down another — a malformed source naming the same parent
/// twice would otherwise recurse forever.
fn nest_depth(boundaries: &[Boundary], alias: Option<&str>, seen: &mut HashSet<String>) -> usize {
    let mut deepest = 0;
    for b in boundaries {
        if b.parent.as_deref() != alias || seen.contains(&b.alias) {
            continue;
        }
        seen.insert(b.alias.clone());
        deepest = deepest.max(1 + nest_depth(boundaries, Some(&b.alias), seen));
    }
    deepest
}

/// The gutters between blocks, widened for nesting.
///
/// Boundary frames hang outside their content and into the gutter — that is what
/// keeps members on the shared column grid. The gutters therefore have to be wide
/// enough for two neighbouring frames to overhang into the same gap without
/// touching, and deeper nesting means a bigger overhang. Each level adds a ring of
/// padding on every side, and a label strip on top as well, so the vertical
/// clearance two stacked frames need grows faster than the horizontal.
pub fn gutters(boundaries: &[Boundary]) -> (f64, f64) {
    let depth = count(nest_depth(boundaries, None, &mut HashSet::new()).max(1));
    (
        l::GAP_X.max(depth * 2.0 * l::BOUNDARY_PAD + 20.0),
        l::GAP_Y.max(depth * (2.0 * l::BOUNDARY_PAD + l::BOUNDARY_LABEL_H) + 20.0),
    )
}

/// A laid-out group: direct elements plus nested boundary frames, all relative to
/// the group's own origin.
struct GroupBox {
    width: f64,
    height: f64,
    /// Shell index and its offset within the group.
    elements: Vec<(usize, f64, f64)>,
    kids: Vec<Kid>,
}

struct Kid {
    /// Index into the diagram's boundary list.
    boundary: usize,
    box_: GroupBox,
    /// Nesting depth of the *enclosing* group, which is what the renderer insets
    /// the frame by.
    depth: usize,
    dx: f64,
    dy: f64,
    width: f64,
    height: f64,
}

/// Members and child boundaries, grouped by their parent in declaration order.
struct Tree {
    members: Vec<(Option<String>, Vec<usize>)>,
    children: Vec<(Option<String>, Vec<usize>)>,
}

fn push_into(map: &mut Vec<(Option<String>, Vec<usize>)>, key: Option<String>, value: usize) {
    match map.iter_mut().find(|(k, _)| *k == key) {
        Some((_, list)) => list.push(value),
        None => map.push((key, vec![value])),
    }
}

fn take<'a>(map: &'a [(Option<String>, Vec<usize>)], key: Option<&str>) -> &'a [usize] {
    map.iter()
        .find(|(k, _)| k.as_deref() == key)
        .map_or(&[], |(_, list)| list.as_slice())
}

impl Tree {
    /// An element or boundary naming a parent that was never declared belongs to
    /// the root, rather than disappearing into a group that does not exist.
    fn build(diagram: &Diagram, shells: &[Shell]) -> Self {
        let known: HashSet<&str> = diagram
            .boundaries
            .iter()
            .map(|b| b.alias.as_str())
            .collect();
        let parent_of = |alias: Option<&String>| -> Option<String> {
            alias.filter(|a| known.contains(a.as_str())).cloned()
        };
        let mut members = Vec::new();
        for (i, shell) in shells.iter().enumerate() {
            let boundary = diagram
                .elements
                .get(shell.index)
                .and_then(|e| e.boundary.as_ref());
            push_into(&mut members, parent_of(boundary), i);
        }
        let mut children = Vec::new();
        for (i, b) in diagram.boundaries.iter().enumerate() {
            push_into(&mut children, parent_of(b.parent.as_ref()), i);
        }
        Self { members, children }
    }
}

/// Size one group: its own elements stacked above its nested boundaries.
fn size_group(
    tree: &Tree,
    box_size: Size,
    diagram: &Diagram,
    alias: Option<&str>,
    depth: usize,
    seen: &mut HashSet<String>,
    gaps: (f64, f64),
) -> GroupBox {
    let (gap_x, gap_y) = gaps;
    let direct = take(&tree.members, alias);
    let kids: Vec<Kid> = take(&tree.children, alias)
        .iter()
        .filter_map(|&bi| {
            let b = diagram.boundaries.get(bi)?;
            if !seen.insert(b.alias.clone()) {
                return None;
            }
            let box_ = size_group(
                tree,
                box_size,
                diagram,
                Some(&b.alias),
                depth + 1,
                seen,
                gaps,
            );
            Some(Kid {
                boundary: bi,
                depth,
                dx: 0.0,
                dy: 0.0,
                // Packed by *content* size. The frame is derived afterwards and
                // hangs into the surrounding gutter, so a boundary never pushes
                // its members off the shared column grid — which is what the edge
                // router routes on.
                width: box_.width,
                height: box_.height,
                box_,
            })
        })
        .collect();

    let elem_pack = pack_rows(
        &vec![box_size; direct.len()],
        diagram.config.shape_in_row,
        gap_x,
        gap_y,
    );
    let kid_pack = pack_rows(
        &kids
            .iter()
            .map(|k| Size::new(k.width, k.height))
            .collect::<Vec<_>>(),
        diagram.config.boundary_in_row,
        gap_x,
        gap_y,
    );

    let width = elem_pack.width.max(kid_pack.width);
    let gap = if !direct.is_empty() && !kids.is_empty() {
        gap_y
    } else {
        0.0
    };
    let kid_off_y = elem_pack.height + gap;

    GroupBox {
        width,
        height: elem_pack.height + gap + kid_pack.height,
        // Bands are left-aligned for the same reason rows are: a shared column
        // grid.
        elements: direct
            .iter()
            .enumerate()
            .filter_map(|(i, &shell)| {
                let at = elem_pack.positions.get(i)?;
                Some((shell, at.x, at.y))
            })
            .collect(),
        kids: kids
            .into_iter()
            .enumerate()
            .map(|(i, mut k)| {
                if let Some(at) = kid_pack.positions.get(i) {
                    k.dx = at.x;
                    k.dy = kid_off_y + at.y;
                }
                k
            })
            .collect(),
    }
}

/// Everything the placement pass produces.
pub struct Placement {
    pub elements: Vec<PlacedElement>,
    pub boundaries: Vec<PlacedBoundary>,
}

/// Turn a sized group tree into absolute geometry.
fn emit(
    group: &GroupBox,
    ox: f64,
    oy: f64,
    diagram: &Diagram,
    shells: &[Shell],
    box_size: Size,
    out: &mut Placement,
) {
    for &(shell_index, dx, dy) in &group.elements {
        let Some(shell) = shells.get(shell_index) else {
            continue;
        };
        let Some(el) = diagram.elements.get(shell.index) else {
            continue;
        };
        out.elements.push(PlacedElement {
            alias: el.alias.clone(),
            kind: el.kind,
            variant: el.variant,
            tag: shell.tag.to_string(),
            label: el.label.clone(),
            techn: el.techn.clone(),
            descr: shell.descr.clone(),
            external: el.external,
            rect: Rect::new(ox + dx, oy + dy, box_size.width, box_size.height),
        });
    }
    for kid in &group.kids {
        let (x, y) = (ox + kid.dx, oy + kid.dy);
        if let Some(b) = diagram.boundaries.get(kid.boundary) {
            // Placeholder: the real frame is fitted to the content once it is
            // placed.
            out.boundaries.push(PlacedBoundary {
                alias: b.alias.clone(),
                label: b.label.clone(),
                kind: b.kind,
                depth: kid.depth,
                rect: Rect::new(x, y, kid.width, kid.height),
            });
        }
        emit(&kid.box_, x, y, diagram, shells, box_size, out);
    }
}

/// Fit every boundary frame to the content it encloses.
///
/// Deepest first, so a parent's frame can wrap the frames of its children rather
/// than just their raw boxes. Each frame hangs `BOUNDARY_PAD` outside its content
/// (plus a label strip on top), which lands in the gutter.
fn fit_boundaries(diagram: &Diagram, placed: &mut Placement) {
    let known: HashSet<&str> = diagram
        .boundaries
        .iter()
        .map(|b| b.alias.as_str())
        .collect();
    // Keyed by alias, never by index: the placed elements are in emit order,
    // group by group, while the source is in declaration order — so zipping them
    // by position silently assigns members to the wrong boundary.
    let owned = |alias: &str| -> Vec<Rect> {
        diagram
            .elements
            .iter()
            .filter(|src| {
                src.boundary
                    .as_deref()
                    .is_some_and(|b| known.contains(b) && b == alias)
            })
            .filter_map(|src| {
                placed
                    .elements
                    .iter()
                    .find(|e| e.alias == src.alias)
                    .map(|e| e.rect)
            })
            .collect()
    };

    let mut order: Vec<usize> = (0..placed.boundaries.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(placed.boundaries.get(i).map_or(0, |b| b.depth)));

    for i in order {
        let Some(alias) = placed.boundaries.get(i).map(|b| b.alias.clone()) else {
            continue;
        };
        let mut parts = owned(&alias);
        parts.extend(
            placed
                .boundaries
                .iter()
                .filter(|child| {
                    diagram
                        .boundaries
                        .iter()
                        .find(|d| d.alias == child.alias)
                        .and_then(|d| d.parent.as_deref())
                        == Some(alias.as_str())
                })
                .map(|child| child.rect),
        );
        if parts.is_empty() {
            continue;
        }
        let x0 = parts.iter().map(|p| p.x).fold(f64::INFINITY, f64::min) - l::BOUNDARY_PAD;
        let y0 = parts.iter().map(|p| p.y).fold(f64::INFINITY, f64::min)
            - l::BOUNDARY_PAD
            - l::BOUNDARY_LABEL_H;
        let x1 = parts
            .iter()
            .map(Rect::right)
            .fold(f64::NEG_INFINITY, f64::max)
            + l::BOUNDARY_PAD;
        let y1 = parts
            .iter()
            .map(Rect::bottom)
            .fold(f64::NEG_INFINITY, f64::max)
            + l::BOUNDARY_PAD;
        if let Some(frame) = placed.boundaries.get_mut(i) {
            frame.rect = Rect::new(x0, y0, x1 - x0, y1 - y0);
        }
    }
}

/// Place every element and boundary, with the origin at `(ox, oy)`.
pub fn place(diagram: &Diagram, shells: &[Shell], box_size: Size, ox: f64, oy: f64) -> Placement {
    let tree = Tree::build(diagram, shells);
    let gaps = gutters(&diagram.boundaries);
    let root = size_group(&tree, box_size, diagram, None, 0, &mut HashSet::new(), gaps);
    let mut out = Placement {
        elements: Vec::new(),
        boundaries: Vec::new(),
    };
    emit(&root, ox, oy, diagram, shells, box_size, &mut out);
    fit_boundaries(diagram, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::c4::parse;

    fn diagram(source: &str) -> Diagram {
        parse(source)
    }

    #[test]
    fn a_tag_names_the_kind_and_marks_the_external_ones() {
        let d = diagram(
            "C4Context\nPerson(a,\"A\")\nPerson_Ext(b,\"B\")\nSystem(c,\"C\")\nSystem_Ext(d,\"D\")\nContainer(e,\"E\",\"Rust\")\nComponent(f,\"F\",\"Rust\")",
        );
        let tags: Vec<&str> = d.elements.iter().map(kind_tag).collect();
        assert_eq!(
            tags,
            vec![
                "«Person»",
                "«External Person»",
                "«System»",
                "«External System»",
                "«Container»",
                "«Component»"
            ]
        );
    }

    #[test]
    fn every_box_takes_one_size_wide_enough_for_the_longest_label() {
        let d = diagram("C4Context\nSystem(a,\"A\")\nSystem(b,\"A considerably longer name\")");
        let (size, shells) = size_boxes(&d.elements);
        assert!(size.width > l::BOX_MIN_W, "{size:?}");
        assert!(size.width <= l::BOX_MAX_W);
        assert_eq!(shells.len(), 2);
    }

    #[test]
    fn a_very_long_label_is_capped_rather_than_stretching_the_grid() {
        let long = "x".repeat(400);
        let d = diagram(&format!("C4Context\nSystem(a,\"{long}\")"));
        let (size, _) = size_boxes(&d.elements);
        assert!((size.width - l::BOX_MAX_W).abs() < 1e-9);
    }

    #[test]
    fn a_description_grows_the_box_and_wraps_inside_it() {
        let plain = diagram("C4Context\nSystem(a,\"A\")");
        let described = diagram(
            "C4Context\nSystem(a,\"A\",\"A description long enough to need more than a single line of its own\")",
        );
        let (bare, _) = size_boxes(&plain.elements);
        let (grown, shells) = size_boxes(&described.elements);
        assert!(grown.height > bare.height);
        assert!(shells[0].descr.len() > 1, "{:?}", shells[0].descr);
        assert!(shells[0].descr.len() <= l::DESCR_MAX_LINES);
    }

    #[test]
    fn a_technology_line_grows_the_box_too() {
        let (bare, _) = size_boxes(&diagram("C4Context\nSystem(a,\"A\")").elements);
        let (with, _) = size_boxes(&diagram("C4Container\nContainer(a,\"A\",\"Rust\")").elements);
        assert!((with.height - bare.height - l::TECHN_H).abs() < 1e-9);
    }

    #[test]
    fn gutters_widen_with_nesting() {
        // Even a flat diagram is sized for one level: a boundary could still be
        // added without the grid changing under it.
        let flat = diagram("C4Context\nSystem(a,\"A\")");
        let (flat_x, flat_y) = gutters(&flat.boundaries);
        assert!((flat_x - l::GAP_X).abs() < 1e-9);
        assert!(flat_y > l::GAP_Y);
        let nested = diagram(
            "C4Deployment\nDeployment_Node(a,\"A\"){\nDeployment_Node(b,\"B\"){\nDeployment_Node(c,\"C\"){\nSystem(s,\"S\")\n}\n}\n}",
        );
        let (gx, gy) = gutters(&nested.boundaries);
        assert!(gx > flat_x, "{gx}");
        assert!(gy > flat_y, "{gy}");
    }

    #[test]
    fn a_boundary_frame_wraps_only_its_own_members() {
        let d = diagram(
            "C4Context\nSystem(loose,\"Loose\")\nSystem_Boundary(bnd,\"Grouped\"){\nSystem(inner,\"Inner\")\n}",
        );
        let (size, shells) = size_boxes(&d.elements);
        let placed = place(&d, &shells, size, l::PADDING, l::PADDING);
        let frame = &placed.boundaries[0].rect;
        let inner = placed
            .elements
            .iter()
            .find(|e| e.alias == "inner")
            .map(|e| e.rect)
            .expect("inner placed");
        let loose = placed
            .elements
            .iter()
            .find(|e| e.alias == "loose")
            .map(|e| e.rect)
            .expect("loose placed");
        // The member is inside the frame ...
        assert!(frame.x <= inner.x && frame.right() >= inner.right());
        assert!(frame.y <= inner.y && frame.bottom() >= inner.bottom());
        // ... and the non-member is not captured by it.
        assert!(
            loose.right() <= frame.x || loose.x >= frame.right() || loose.bottom() <= frame.y,
            "{loose:?} fell inside {frame:?}"
        );
    }

    #[test]
    fn a_nested_frame_sits_inside_its_parent() {
        let d = diagram(
            "C4Deployment\nDeployment_Node(outer,\"Outer\"){\nDeployment_Node(inner,\"Inner\"){\nSystem(s,\"S\")\n}\n}",
        );
        let (size, shells) = size_boxes(&d.elements);
        let placed = place(&d, &shells, size, l::PADDING, l::PADDING);
        let outer = placed
            .boundaries
            .iter()
            .find(|b| b.alias == "outer")
            .map(|b| b.rect)
            .expect("outer placed");
        let inner = placed
            .boundaries
            .iter()
            .find(|b| b.alias == "inner")
            .map(|b| b.rect)
            .expect("inner placed");
        assert!(
            outer.x < inner.x && outer.right() > inner.right(),
            "{outer:?} {inner:?}"
        );
        assert!(outer.y < inner.y && outer.bottom() > inner.bottom());
    }

    #[test]
    fn an_element_naming_an_undeclared_boundary_lands_at_the_root() {
        let d = diagram("C4Context\nSystem(a,\"A\")");
        let mut d = d;
        d.elements[0].boundary = Some("nowhere".into());
        let (size, shells) = size_boxes(&d.elements);
        let placed = place(&d, &shells, size, 10.0, 20.0);
        assert_eq!(placed.elements.len(), 1);
        assert!(placed.boundaries.is_empty());
        assert!((placed.elements[0].rect.x - 10.0).abs() < 1e-9);
        assert!((placed.elements[0].rect.y - 20.0).abs() < 1e-9);
    }

    #[test]
    fn an_empty_boundary_keeps_its_placeholder_frame() {
        let d = diagram("C4Context\nSystem_Boundary(bnd,\"Empty\"){\n}\nSystem(a,\"A\")");
        let (size, shells) = size_boxes(&d.elements);
        let placed = place(&d, &shells, size, 0.0, 0.0);
        assert_eq!(placed.boundaries.len(), 1);
        // Nothing to fit to, so it keeps the zero-content box it was emitted at.
        assert!(placed.boundaries[0].rect.width.abs() < 1e-9);
    }

    #[test]
    fn a_diagram_with_nothing_in_it_places_nothing() {
        let d = diagram("C4Context");
        let (size, shells) = size_boxes(&d.elements);
        let placed = place(&d, &shells, size, 0.0, 0.0);
        assert!(placed.elements.is_empty());
        assert!(placed.boundaries.is_empty());
    }

    #[test]
    fn rows_honour_the_authors_width() {
        let d = diagram(
            "C4Context\nUpdateLayoutConfig($c4ShapeInRow=\"2\")\nSystem(a,\"A\")\nSystem(b,\"B\")\nSystem(c,\"C\")",
        );
        let (size, shells) = size_boxes(&d.elements);
        let placed = place(&d, &shells, size, 0.0, 0.0);
        let ys: Vec<f64> = placed.elements.iter().map(|e| e.rect.y).collect();
        assert!((ys[0] - ys[1]).abs() < 1e-9, "{ys:?}");
        assert!(ys[2] > ys[0], "{ys:?}");
    }
}
