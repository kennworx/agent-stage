//! Where the skeleton goes.
//!
//! A horizontal spine runs left to right into the effect's box. Categories
//! attach at evenly spaced points along it and angle outward, alternating above
//! and below. Causes hang off their category's bone as horizontal sub-bones, and
//! nested ones step further out again.
//!
//! Everything is laid out about a spine on `y = 0` while a bounding box is
//! accumulated, then the whole drawing is moved so its top-left is the padding.

use crate::round::count;
use crate::scene::Point;

use super::types::{Cause, Diagram};

pub const PADDING: f64 = 28.0;
/// Spine to the left of the first category.
pub const TAIL_STUB: f64 = 54.0;
pub const CATEGORY_GAP: f64 = 200.0;
/// From the last category's attachment to the head box.
pub const HEAD_GAP: f64 = 72.0;
pub const BONE_ANGLE_DEG: f64 = 62.0;
pub const BASE_BONE_LEN: f64 = 92.0;
/// Added to a category's bone for each cause it carries.
pub const CAUSE_STEP: f64 = 34.0;
pub const CAUSE_LINE_LEN: f64 = 70.0;
pub const CAUSE_LABEL_GAP: f64 = 8.0;
pub const CAUSE_ROW_H: f64 = 26.0;
pub const CAUSE_INDENT: f64 = 22.0;
pub const CAT_LABEL_GAP: f64 = 12.0;
pub const CAT_FONT: f64 = 14.0;
pub const CAT_WEIGHT: u32 = 600;
pub const CAUSE_FONT: f64 = 12.0;
pub const CAUSE_WEIGHT: u32 = 400;
pub const EFFECT_FONT: f64 = 16.0;
pub const EFFECT_WEIGHT: u32 = 700;
pub const HEAD_PAD_X: f64 = 16.0;
pub const HEAD_PAD_Y: f64 = 11.0;

/// The effect, in its box at the right-hand end of the spine.
#[derive(Debug, Clone, PartialEq)]
pub struct Head {
    pub id: String,
    pub text: String,
    pub at: Point,
    pub box_at: Point,
    pub box_width: f64,
    pub box_height: f64,
}

/// One cause, on its own horizontal sub-bone.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedCause {
    pub id: String,
    pub text: String,
    /// Whatever it hangs off: a category, or another cause.
    pub parent_id: String,
    pub bone: (Point, Point),
    pub label_at: Point,
}

/// One category, on a bone angled off the spine.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedCategory {
    pub id: String,
    pub text: String,
    pub bone: (Point, Point),
    pub label_at: Point,
    pub above: bool,
    /// Flattened: nested causes are in here too, each naming its parent.
    pub causes: Vec<PlacedCause>,
}

/// A laid-out fishbone.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub spine: Option<(Point, Point)>,
    pub head: Option<Head>,
    pub categories: Vec<PlacedCategory>,
}

/// The extent of everything drawn, grown a point at a time.
struct Bounds {
    min: Point,
    max: Point,
}

impl Bounds {
    /// Starts at the origin rather than empty, because the spine's own tail is
    /// there and nothing else would put a point on it.
    fn new() -> Self {
        Self {
            min: Point::new(0.0, 0.0),
            max: Point::new(0.0, 0.0),
        }
    }

    fn track(&mut self, x: f64, y: f64) {
        self.min = Point::new(self.min.x.min(x), self.min.y.min(y));
        self.max = Point::new(self.max.x.max(x), self.max.y.max(y));
    }
}

/// Hands out an id per node, disambiguating anything already claimed.
struct Ids(Vec<String>);

impl Ids {
    fn take(&mut self, text: &str) -> String {
        let root = if text.trim().is_empty() {
            "node"
        } else {
            text.trim()
        };
        let mut id = root.to_string();
        let mut n = 2usize;
        while self.0.contains(&id) {
            id = format!("{root}-{n}");
            n += 1;
        }
        self.0.push(id.clone());
        id
    }
}

/// Place one cause and, below it, everything that causes it.
fn place_cause(
    cause: &Cause,
    at: Point,
    dir: f64,
    parent_id: &str,
    ids: &mut Ids,
    bounds: &mut Bounds,
    out: &mut Vec<PlacedCause>,
) {
    let end = Point::new(at.x - CAUSE_LINE_LEN, at.y);
    let id = ids.take(&cause.text);
    let label_at = Point::new(end.x - CAUSE_LABEL_GAP, end.y);
    let width = crate::metrics::text_width(&cause.text, CAUSE_FONT, CAUSE_WEIGHT);
    out.push(PlacedCause {
        id: id.clone(),
        text: cause.text.clone(),
        parent_id: parent_id.to_string(),
        bone: (at, end),
        label_at,
    });
    bounds.track(at.x, at.y);
    bounds.track(end.x, end.y);
    // The label runs leftward from its anchor, so its far edge is what bounds.
    bounds.track(label_at.x - width, label_at.y - CAUSE_FONT / 2.0);
    bounds.track(label_at.x, label_at.y + CAUSE_FONT / 2.0);

    // Sub-causes stack further from the spine, indented past their parent.
    let mut y = end.y + dir * CAUSE_ROW_H;
    for sub in &cause.causes {
        place_cause(
            sub,
            Point::new(end.x - CAUSE_INDENT, y),
            dir,
            &id,
            ids,
            bounds,
            out,
        );
        y += dir * CAUSE_ROW_H;
    }
}

/// Where along a category's bone its `index`th cause branches off.
///
/// A lone cause sits well down the bone rather than at its root; several spread
/// evenly over the same stretch.
fn cause_position(index: usize, total: usize) -> f64 {
    if total <= 1 {
        return 0.6;
    }
    0.32 + 0.6 * count(index) / (count(total) - 1.0)
}

/// Move every point by `offset`, which is the last thing layout does.
fn shift(placed: &mut Placed, offset: Point) {
    let move_point = |p: Point| Point::new(p.x + offset.x, p.y + offset.y);
    if let Some((a, b)) = placed.spine {
        placed.spine = Some((move_point(a), move_point(b)));
    }
    if let Some(head) = &mut placed.head {
        head.at = move_point(head.at);
        head.box_at = move_point(head.box_at);
    }
    for category in &mut placed.categories {
        category.bone = (move_point(category.bone.0), move_point(category.bone.1));
        category.label_at = move_point(category.label_at);
        for cause in &mut category.causes {
            cause.bone = (move_point(cause.bone.0), move_point(cause.bone.1));
            cause.label_at = move_point(cause.label_at);
        }
    }
}

/// Lay out a parsed fishbone.
pub fn layout(diagram: &Diagram) -> Placed {
    let angle = BONE_ANGLE_DEG.to_radians();
    let (cos, sin) = (angle.cos(), angle.sin());
    let mut ids = Ids(Vec::new());
    let mut bounds = Bounds::new();

    let n = diagram.categories.len();
    // The effect claims its id first, so a category sharing its name is the one
    // that gets disambiguated.
    let effect_id = ids.take(&diagram.effect);

    let attach_x = |i: usize| TAIL_STUB + count(i) * CATEGORY_GAP;
    let head_left = if n > 0 { attach_x(n - 1) } else { TAIL_STUB } + HEAD_GAP;
    bounds.track(head_left, 0.0);

    let text_width = crate::metrics::text_width(&diagram.effect, EFFECT_FONT, EFFECT_WEIGHT);
    let box_width = text_width + HEAD_PAD_X * 2.0;
    let box_height = EFFECT_FONT + HEAD_PAD_Y * 2.0;
    let head = Head {
        id: effect_id.clone(),
        text: diagram.effect.clone(),
        at: Point::new(head_left + box_width / 2.0, 0.0),
        box_at: Point::new(head_left, -box_height / 2.0),
        box_width,
        box_height,
    };
    bounds.track(head.box_at.x, head.box_at.y);
    bounds.track(head.box_at.x + box_width, head.box_at.y + box_height);

    let categories: Vec<PlacedCategory> = diagram
        .categories
        .iter()
        .enumerate()
        .map(|(i, category)| {
            // Alternating sides, starting above, is what makes it a fishbone
            // rather than a comb.
            let above = i % 2 == 0;
            let dir = if above { -1.0 } else { 1.0 };
            let attach = Point::new(attach_x(i), 0.0);

            let total = category.causes.len();
            let bone_len = BASE_BONE_LEN + count(total) * CAUSE_STEP;
            let end = Point::new(attach.x - bone_len * cos, attach.y + dir * bone_len * sin);

            let id = ids.take(&category.text);
            let label_at = Point::new(
                end.x,
                if above {
                    end.y - CAT_LABEL_GAP
                } else {
                    end.y + CAT_LABEL_GAP + CAT_FONT
                },
            );
            let width = crate::metrics::text_width(&category.text, CAT_FONT, CAT_WEIGHT);
            bounds.track(attach.x, attach.y);
            bounds.track(end.x, end.y);
            bounds.track(
                label_at.x - width / 2.0,
                if above {
                    label_at.y - CAT_FONT
                } else {
                    label_at.y
                },
            );
            bounds.track(label_at.x + width / 2.0, label_at.y);

            let mut causes = Vec::new();
            for (ci, cause) in category.causes.iter().enumerate() {
                let t = cause_position(ci, total);
                place_cause(
                    cause,
                    Point::new(
                        attach.x - bone_len * cos * t,
                        attach.y + dir * bone_len * sin * t,
                    ),
                    dir,
                    &id,
                    &mut ids,
                    &mut bounds,
                    &mut causes,
                );
            }

            PlacedCategory {
                id,
                text: category.text.clone(),
                bone: (attach, end),
                label_at,
                above,
                causes,
            }
        })
        .collect();

    let mut placed = Placed {
        width: (bounds.max.x - bounds.min.x + PADDING * 2.0).ceil(),
        height: (bounds.max.y - bounds.min.y + PADDING * 2.0).ceil(),
        spine: Some((Point::new(0.0, 0.0), Point::new(head_left, 0.0))),
        head: Some(head),
        categories,
    };
    shift(
        &mut placed,
        Point::new(PADDING - bounds.min.x, PADDING - bounds.min.y),
    );
    placed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ishikawa::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    const FISH: &str = "ishikawa\n\
        Late delivery\n  \
          People\n    \
            Understaffed\n  \
          Process\n    \
            Manual steps\n    \
            Handoffs";

    #[test]
    fn categories_alternate_above_and_below_the_spine() {
        let out = placed(FISH);
        let sides: Vec<bool> = out.categories.iter().map(|c| c.above).collect();
        assert_eq!(sides, [true, false]);
        let (_, spine_end) = out.spine.expect("a spine");
        assert!(out.categories[0].bone.1.y < spine_end.y, "first bone above");
        assert!(out.categories[1].bone.1.y > spine_end.y, "second below");
    }

    #[test]
    fn categories_attach_at_even_intervals_along_the_spine() {
        let out = placed(FISH);
        let gap = out.categories[1].bone.0.x - out.categories[0].bone.0.x;
        assert!((gap - CATEGORY_GAP).abs() < 1e-9);
        // And they all attach on the spine itself.
        let (a, _) = out.spine.expect("a spine");
        assert!(out
            .categories
            .iter()
            .all(|c| (c.bone.0.y - a.y).abs() < 1e-9));
    }

    #[test]
    fn a_category_bone_grows_with_the_causes_it_carries() {
        let bare = placed("ishikawa\nE\n  A");
        let loaded = placed("ishikawa\nE\n  A\n    one\n    two");
        let length = |p: &Placed| {
            let bone = &p.categories[0].bone;
            (bone.1.x - bone.0.x).hypot(bone.1.y - bone.0.y)
        };
        assert!((length(&loaded) - length(&bare) - CAUSE_STEP * 2.0).abs() < 1e-6);
    }

    #[test]
    fn a_lone_cause_sits_down_the_bone_rather_than_at_its_root() {
        assert!((cause_position(0, 1) - 0.6).abs() < 1e-9);
        // Several spread evenly over the same stretch.
        assert!((cause_position(0, 3) - 0.32).abs() < 1e-9);
        assert!((cause_position(2, 3) - 0.92).abs() < 1e-9);
    }

    #[test]
    fn a_cause_runs_leftward_and_is_named_past_its_end() {
        let out = placed(FISH);
        let cause = &out.categories[0].causes[0];
        assert!((cause.bone.1.x - (cause.bone.0.x - CAUSE_LINE_LEN)).abs() < 1e-9);
        assert!((cause.bone.0.y - cause.bone.1.y).abs() < 1e-9, "horizontal");
        assert!(cause.label_at.x < cause.bone.1.x);
    }

    #[test]
    fn a_nested_cause_steps_further_out_and_names_its_parent() {
        let out = placed("ishikawa\nE\n  Cat\n    Cause\n      Sub");
        let causes = &out.categories[0].causes;
        assert_eq!(causes.len(), 2, "flattened, parent then child");
        assert_eq!(causes[1].parent_id, causes[0].id);
        // Indented past its parent's end, and one row further from the spine.
        assert!(causes[1].bone.0.x < causes[0].bone.1.x);
        assert!((causes[1].bone.0.y - causes[0].bone.1.y).abs() > 1e-9);
    }

    #[test]
    fn a_sub_cause_moves_away_from_the_spine_on_whichever_side_it_is() {
        let out = placed("ishikawa\nE\n  Above\n    C\n      S\n  Below\n    C2\n      S2");
        let above = &out.categories[0].causes;
        let below = &out.categories[1].causes;
        assert!(above[1].bone.0.y < above[0].bone.0.y, "upward above");
        assert!(below[1].bone.0.y > below[0].bone.0.y, "downward below");
    }

    #[test]
    fn the_effect_gets_a_box_at_the_right_hand_end_of_the_spine() {
        let out = placed(FISH);
        let head = out.head.clone().expect("a head");
        let (_, spine_end) = out.spine.expect("a spine");
        assert!((head.box_at.x - spine_end.x).abs() < 1e-9);
        // The name is centred in the box.
        assert!((head.at.x - (head.box_at.x + head.box_width / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn the_head_box_widens_with_the_name_in_it() {
        let short = placed("ishikawa\nE").head.expect("a head").box_width;
        let long = placed("ishikawa\nA much longer effect indeed")
            .head
            .expect("a head")
            .box_width;
        assert!(long > short);
    }

    #[test]
    fn the_whole_drawing_fits_on_the_canvas_with_its_padding() {
        let out = placed(FISH);
        let (a, b) = out.spine.expect("a spine");
        assert!(a.x >= PADDING - 1e-9);
        assert!(b.x <= out.width - PADDING + 1e-9);
        for category in &out.categories {
            assert!(category.bone.1.y >= 0.0, "{:?}", category.bone);
            assert!(category.bone.1.y <= out.height, "{:?}", category.bone);
        }
    }

    #[test]
    fn two_nodes_named_the_same_still_get_distinct_ids() {
        let out = placed("ishikawa\nSame\n  Same\n  Same");
        let ids: Vec<&str> = out.categories.iter().map(|c| c.id.as_str()).collect();
        // The effect claimed `Same` first, so both categories are numbered.
        assert_eq!(ids, ["Same-2", "Same-3"]);
    }

    #[test]
    fn an_empty_diagram_still_has_a_spine() {
        let out = placed("ishikawa");
        assert!(out.categories.is_empty());
        assert!(out.spine.is_some());
        assert!(out.width > 0.0);
    }
}
