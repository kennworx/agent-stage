//! Where each frame sits: one column per time frame, three swimlane rows.
//!
//! Columns run in number order, not in the order the lines were written, so a
//! model written out of sequence still reads left to right.

use crate::round::count;
use crate::scene::Point;

use super::types::{Entity, Frame, Lane, Model};

pub const PADDING: f64 = 32.0;
/// The left gutter the lane names sit in.
pub const LANE_LABEL_WIDTH: f64 = 150.0;
/// The strip of frame numbers above the lanes.
pub const AXIS_HEIGHT: f64 = 28.0;
pub const TITLE_HEIGHT: f64 = 40.0;
pub const TITLE_FONT: f64 = 18.0;
pub const COL_GAP: f64 = 28.0;
pub const MIN_COL_WIDTH: f64 = 120.0;
pub const BOX_HEIGHT: f64 = 56.0;
/// Above and below a box within its lane.
pub const LANE_PAD_V: f64 = 18.0;
/// A box is inset from its column by this much on each side.
pub const BOX_INSET_X: f64 = 8.0;
pub const LABEL_FONT: f64 = 13.0;
pub const LABEL_WEIGHT: u32 = 500;

/// One swimlane band.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedLane {
    pub lane: Lane,
    pub y: f64,
    pub height: f64,
    pub label_at: Point,
}

/// One frame's box.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedFrame {
    pub id: String,
    pub entity: Entity,
    pub name: String,
    pub at: Point,
    pub width: f64,
    pub height: f64,
}

impl PlacedFrame {
    fn centre(&self) -> Point {
        Point::new(self.at.x + self.width / 2.0, self.at.y + self.height / 2.0)
    }
}

/// One inferred connector between consecutive frames.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedRelation {
    pub from: String,
    pub to: String,
    pub a: Point,
    pub b: Point,
}

/// A laid-out event model.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<(String, Point)>,
    pub lanes: Vec<PlacedLane>,
    /// The frame numbers along the top.
    pub axis: Vec<(String, Point)>,
    pub frames: Vec<PlacedFrame>,
    pub relations: Vec<PlacedRelation>,
}

/// Where the diagram's name sits: the middle of the band reserved above it.
fn title_baseline() -> f64 {
    f64::midpoint(PADDING, TITLE_HEIGHT)
}

/// Where the ray from the centre of `rect` toward `towards` leaves the border.
fn border_point(rect: &PlacedFrame, towards: Point) -> Point {
    let centre = rect.centre();
    let dx = towards.x - centre.x;
    let dy = towards.y - centre.y;
    if dx == 0.0 && dy == 0.0 {
        return centre;
    }
    let scale_x = if dx == 0.0 {
        f64::INFINITY
    } else {
        rect.width / 2.0 / dx.abs()
    };
    let scale_y = if dy == 0.0 {
        f64::INFINITY
    } else {
        rect.height / 2.0 / dy.abs()
    };
    let scale = scale_x.min(scale_y);
    Point::new(centre.x + dx * scale, centre.y + dy * scale)
}

/// The frames in column order: by number, and by where they were written when
/// two numbers are equal.
fn ordered(model: &Model) -> Vec<&Frame> {
    let mut out: Vec<(usize, &Frame)> = model.frames.iter().enumerate().collect();
    out.sort_by_key(|(i, frame)| (frame.numeric, *i));
    out.into_iter().map(|(_, frame)| frame).collect()
}

/// Lay out a parsed event model.
pub fn layout(model: &Model) -> Placed {
    let frames_in_order = ordered(model);
    let lane_height = BOX_HEIGHT + LANE_PAD_V * 2.0;

    // Every column is as wide as the longest name, so the grid stays a grid.
    let widest = frames_in_order
        .iter()
        .map(|f| crate::metrics::text_width(&f.name, LABEL_FONT, LABEL_WEIGHT))
        .fold(0.0_f64, f64::max);
    let col_width = MIN_COL_WIDTH.max(widest.ceil() + 32.0);

    let grid_left = PADDING + LANE_LABEL_WIDTH;
    let top = PADDING
        + if model.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        }
        + AXIS_HEIGHT;
    let lane_top = |index: usize| top + count(index) * lane_height;

    let cols = frames_in_order.len();
    let width = grid_left
        + if cols > 0 {
            count(cols) * col_width + (count(cols) - 1.0) * COL_GAP
        } else {
            0.0
        }
        + PADDING;

    let lanes = Lane::ALL
        .into_iter()
        .enumerate()
        .map(|(index, lane)| PlacedLane {
            lane,
            y: lane_top(index),
            height: lane_height,
            label_at: Point::new(PADDING, lane_top(index) + lane_height / 2.0),
        })
        .collect();

    let mut frames = Vec::with_capacity(cols);
    let mut axis = Vec::with_capacity(cols);
    for (column, frame) in frames_in_order.iter().enumerate() {
        let x = grid_left + count(column) * (col_width + COL_GAP);
        let lane = Lane::ALL
            .iter()
            .position(|l| *l == frame.entity.lane())
            .unwrap_or(0);
        frames.push(PlacedFrame {
            id: frame.number.clone(),
            entity: frame.entity,
            name: frame.name.clone(),
            at: Point::new(x + BOX_INSET_X, lane_top(lane) + LANE_PAD_V),
            width: col_width - BOX_INSET_X * 2.0,
            height: BOX_HEIGHT,
        });
        axis.push((
            frame.number.clone(),
            Point::new(x + col_width / 2.0, top - AXIS_HEIGHT / 2.0),
        ));
    }

    // Nothing declares a relation, so the sequence is the relation: each frame
    // leads to the next one along.
    let relations = frames
        .windows(2)
        .filter_map(|pair| {
            let (a, b) = (pair.first()?, pair.get(1)?);
            Some(PlacedRelation {
                from: a.id.clone(),
                to: b.id.clone(),
                a: border_point(a, b.centre()),
                b: border_point(b, a.centre()),
            })
        })
        .collect();

    Placed {
        width,
        height: top + count(Lane::ALL.len()) * lane_height + PADDING,
        title: model
            .title
            .clone()
            .map(|text| (text, Point::new(width / 2.0, title_baseline()))),
        lanes,
        axis,
        frames,
        relations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wire_meets_a_box_on_its_border_whichever_way_it_comes() {
        // The scale is the smaller of the two, so the point lands on whichever
        // side the direction actually reaches first — a wide box met from just
        // above its corner exits the top, not the side.
        let box_ = PlacedFrame {
            id: "a".into(),
            entity: Entity::Command,
            name: "a".into(),
            at: Point::new(0.0, 0.0),
            width: 100.0,
            height: 20.0,
        };
        let centre = box_.centre();
        let hit = |x: f64, y: f64| border_point(&box_, Point::new(x, y));
        // Straight out each side lands on that side's midpoint.
        assert!((hit(centre.x, -50.0).y - box_.at.y).abs() < 1e-9, "top");
        assert!(
            (hit(centre.x, 50.0).y - (box_.at.y + box_.height)).abs() < 1e-9,
            "bottom"
        );
        assert!((hit(-50.0, centre.y).x - box_.at.x).abs() < 1e-9, "left");
        assert!(
            (hit(50.0 + centre.x, centre.y).x - (box_.at.x + box_.width)).abs() < 1e-9,
            "right"
        );
        // Diagonally, the short side is the one that limits it.
        let corner = hit(centre.x + 100.0, centre.y + 100.0);
        assert!(
            (corner.y - (box_.at.y + box_.height)).abs() < 1e-9,
            "{corner:?}"
        );
        // A target on the centre has no direction to leave by.
        assert_eq!(border_point(&box_, centre), centre);
    }
    use crate::eventmodeling::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const MODEL: &str = "eventmodeling\n\
        title Ordering\n\
        tf 01 ui Basket\n\
        tf 02 cmd PlaceOrder\n\
        tf 03 evt OrderPlaced";

    #[test]
    fn columns_run_in_number_order_not_in_the_order_written() {
        let out = placed("eventmodeling\ntf 03 evt Third\ntf 01 ui First\ntf 02 cmd Second");
        let names: Vec<&str> = out.frames.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["First", "Second", "Third"]);
        assert!(out.frames[0].at.x < out.frames[1].at.x);
    }

    #[test]
    fn two_frames_with_the_same_number_keep_the_order_they_were_written() {
        let out = placed("eventmodeling\ntf 1 ui A\ntf 1 cmd B");
        let names: Vec<&str> = out.frames.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["A", "B"]);
    }

    #[test]
    fn each_frame_lands_in_its_own_lane() {
        let out = placed(MODEL);
        let lane_y = |index: usize| out.lanes[index].y;
        assert!((out.frames[0].at.y - (lane_y(0) + LANE_PAD_V)).abs() < 1e-9);
        assert!((out.frames[1].at.y - (lane_y(1) + LANE_PAD_V)).abs() < 1e-9);
        assert!((out.frames[2].at.y - (lane_y(2) + LANE_PAD_V)).abs() < 1e-9);
    }

    #[test]
    fn there_are_always_three_lanes_even_with_nothing_in_them() {
        let out = placed("eventmodeling");
        assert_eq!(out.lanes.len(), 3);
        assert!(out.frames.is_empty());
        // Their names read in a fixed order.
        assert_eq!(out.lanes[0].lane.label(), "UI / Automation");
        assert_eq!(out.lanes[2].lane.label(), "Events");
    }

    #[test]
    fn every_column_is_as_wide_as_the_longest_name() {
        let out = placed("eventmodeling\ntf 1 ui x\ntf 2 cmd A rather long frame name");
        assert!((out.frames[0].width - out.frames[1].width).abs() < 1e-9);
        assert!(out.frames[0].width > MIN_COL_WIDTH - BOX_INSET_X * 2.0);
    }

    #[test]
    fn the_sequence_is_the_relation() {
        let out = placed(MODEL);
        assert_eq!(out.relations.len(), 2);
        assert_eq!(out.relations[0].from, "01");
        assert_eq!(out.relations[0].to, "02");
    }

    #[test]
    fn a_connector_stops_at_a_border_rather_than_a_centre() {
        let out = placed(MODEL);
        let relation = &out.relations[0];
        let from = &out.frames[0];
        // It leaves through the right-hand edge or the bottom, never the middle.
        let inside = relation.a.x > from.at.x + 1e-6
            && relation.a.x < from.at.x + from.width - 1e-6
            && relation.a.y > from.at.y + 1e-6
            && relation.a.y < from.at.y + from.height - 1e-6;
        assert!(!inside, "{relation:?}");
    }

    #[test]
    fn two_frames_sharing_a_centre_give_a_connector_no_direction() {
        let frame = PlacedFrame {
            id: "a".into(),
            entity: Entity::Ui,
            name: "a".into(),
            at: Point::new(0.0, 0.0),
            width: 10.0,
            height: 10.0,
        };
        assert_eq!(border_point(&frame, frame.centre()), frame.centre());
    }

    #[test]
    fn every_frame_is_numbered_along_the_top() {
        let out = placed(MODEL);
        let numbers: Vec<&str> = out.axis.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(numbers, ["01", "02", "03"]);
        assert!(out.axis[0].1.y < out.lanes[0].y);
    }

    #[test]
    fn a_title_pushes_the_axis_down_and_centres_itself() {
        let out = placed(MODEL);
        let (text, at) = out.title.clone().expect("a title");
        assert_eq!(text, "Ordering");
        assert!((at.x - out.width / 2.0).abs() < 1e-9);
        assert!((out.lanes[0].y - (PADDING + TITLE_HEIGHT + AXIS_HEIGHT)).abs() < 1e-9);
    }
}
