//! Where each field sits on the bit grid.
//!
//! Thirty-two bits per row, which is what a packet diagram means by a row. A
//! field crossing that boundary becomes one rectangle per row it occupies —
//! the field is still one thing, so its segments stay inside one identity.

use crate::round::count;
use crate::scene::Point;

use super::types::Diagram;

pub const BITS_PER_ROW: usize = 32;
pub const CELL_WIDTH: f64 = 26.0;
pub const RECT_HEIGHT: f64 = 32.0;
pub const NUMBER_STRIP_H: f64 = 16.0;
pub const ROW_GAP: f64 = 10.0;
pub const PADDING: f64 = 24.0;
pub const TITLE_HEIGHT: f64 = 38.0;
pub const TITLE_FONT: f64 = 18.0;

/// One rectangle of a field: the part of it on a single row.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub at: Point,
    pub width: f64,
    pub height: f64,
    pub start_bit: usize,
    pub end_bit: usize,
    /// Centre anchor for the field's name inside this rectangle.
    pub label_at: Point,
}

/// One field, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedField {
    /// Deterministic and unique within the diagram, derived from the label.
    pub id: String,
    pub label: String,
    pub start: usize,
    pub end: usize,
    pub segments: Vec<Segment>,
}

/// A laid-out packet diagram.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<(String, Point)>,
    pub fields: Vec<PlacedField>,
}

/// A label as an identifier: lower-case, runs of anything else become one dash.
fn slugify(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    for c in label.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

/// A unique id for a field, so two fields sharing a name are still separable.
fn unique_id(label: &str, used: &mut Vec<(String, usize)>) -> String {
    let slug = slugify(label);
    // A label of pure punctuation slugs to nothing, and an element with no id
    // is an element no reviewer can point at.
    let base = if slug.is_empty() {
        "field".to_string()
    } else {
        slug
    };
    if let Some((_, seen)) = used.iter_mut().find(|(b, _)| *b == base) {
        *seen += 1;
        return format!("{base}-{seen}");
    }
    used.push((base.clone(), 1));
    base
}

/// The rectangles one field occupies, one per row it crosses.
fn segments(start: usize, end: usize, top: f64, row_block_h: f64) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut bit = start;
    while bit <= end {
        let row = bit / BITS_PER_ROW;
        let row_last = (row + 1) * BITS_PER_ROW - 1;
        let seg_end = end.min(row_last);
        let col_start = bit - row * BITS_PER_ROW;
        let col_end = seg_end - row * BITS_PER_ROW;

        let x = PADDING + count(col_start) * CELL_WIDTH;
        let width = count(col_end - col_start + 1) * CELL_WIDTH;
        let y = top + count(row) * row_block_h + NUMBER_STRIP_H;
        out.push(Segment {
            at: Point::new(x, y),
            width,
            height: RECT_HEIGHT,
            start_bit: bit,
            end_bit: seg_end,
            label_at: Point::new(x + width / 2.0, y + RECT_HEIGHT / 2.0),
        });
        bit = seg_end + 1;
    }
    out
}

/// Where the diagram's name sits: the middle of the band reserved above it.
///
/// Not one font-size below the padding, which is where a baseline naturally
/// lands and which reads as the title having slipped toward the drawing. The
/// band runs from the top of the canvas to where the content begins, so its
/// middle is the only place that looks deliberate.
fn title_baseline() -> f64 {
    f64::midpoint(PADDING, TITLE_HEIGHT)
}

/// Lay out a parsed packet diagram.
pub fn layout(diagram: &Diagram) -> Placed {
    let top = PADDING
        + if diagram.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        };
    let row_block_h = NUMBER_STRIP_H + RECT_HEIGHT + ROW_GAP;
    let rows = diagram
        .fields
        .iter()
        .map(|f| f.end / BITS_PER_ROW + 1)
        .max()
        .unwrap_or(0);

    let mut used: Vec<(String, usize)> = Vec::new();
    let fields = diagram
        .fields
        .iter()
        .map(|f| PlacedField {
            id: unique_id(&f.label, &mut used),
            label: f.label.clone(),
            start: f.start,
            end: f.end,
            segments: segments(f.start, f.end, top, row_block_h),
        })
        .collect();

    // The grid is always a full thirty-two bits wide, however few are used —
    // a half-width diagram would make two packets look like different scales.
    let width = PADDING * 2.0 + count(BITS_PER_ROW) * CELL_WIDTH;
    let height = if rows > 0 {
        top + count(rows) * row_block_h - ROW_GAP + PADDING
    } else {
        top + PADDING
    };
    Placed {
        width,
        height,
        title: diagram
            .title
            .clone()
            .map(|text| (text, Point::new(width / 2.0, title_baseline()))),
        fields,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::parse;

    #[test]
    fn a_field_inside_one_row_is_one_rectangle() {
        let placed = layout(&parse("packet\n0-7: a"));
        let field = &placed.fields[0];
        assert_eq!(field.segments.len(), 1);
        assert!((field.segments[0].width - 8.0 * CELL_WIDTH).abs() < 1e-9);
        assert!((field.segments[0].at.x - PADDING).abs() < 1e-9);
    }

    #[test]
    fn a_field_crossing_a_row_is_split_but_stays_one_field() {
        let placed = layout(&parse("packet\n24-39: wraps"));
        let field = &placed.fields[0];
        assert_eq!(field.segments.len(), 2);
        assert_eq!(field.segments[0].start_bit, 24);
        assert_eq!(field.segments[0].end_bit, 31);
        assert_eq!(field.segments[1].start_bit, 32);
        assert_eq!(field.segments[1].end_bit, 39);
        // The second row starts back at the left margin, one row lower.
        assert!((field.segments[1].at.x - PADDING).abs() < 1e-9);
        assert!(field.segments[1].at.y > field.segments[0].at.y);
    }

    #[test]
    fn a_field_spanning_several_whole_rows_gets_one_rectangle_each() {
        let placed = layout(&parse("packet\n0-95: long"));
        assert_eq!(placed.fields[0].segments.len(), 3);
    }

    #[test]
    fn the_grid_is_always_a_full_row_wide() {
        let narrow = layout(&parse("packet\n0: a"));
        let wide = layout(&parse("packet\n0-63: a"));
        assert!((narrow.width - wide.width).abs() < 1e-9);
    }

    #[test]
    fn height_follows_the_number_of_rows_used() {
        let one = layout(&parse("packet\n0-31: a"));
        let two = layout(&parse("packet\n0-63: a"));
        assert!(two.height > one.height);
        // An empty diagram is padding alone.
        let empty = layout(&parse("packet"));
        assert!((empty.height - PADDING * 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_title_pushes_the_grid_down_and_centres_itself() {
        let placed = layout(&parse("packet title Frame\n0: a"));
        let (text, at) = placed.title.clone().expect("a title");
        assert_eq!(text, "Frame");
        assert!((at.x - placed.width / 2.0).abs() < 1e-9);
        assert!(placed.fields[0].segments[0].at.y > TITLE_HEIGHT);
    }

    #[test]
    fn an_identity_is_derived_from_the_label_and_stays_unique() {
        let placed = layout(&parse(
            "packet\n0: \"Source Port\"\n1: \"Source Port\"\n2: \"!!!\"",
        ));
        let ids: Vec<&str> = placed.fields.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["source-port", "source-port-2", "field"]);
    }

    #[test]
    fn a_label_of_punctuation_still_gets_an_addressable_id() {
        assert_eq!(layout(&parse("packet\n0: \"- -\"")).fields[0].id, "field");
    }
}
