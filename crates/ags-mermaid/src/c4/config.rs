//! The measurements a C4 drawing is built from.
//!
//! Gathered in one place because the renderer needs the same numbers the layout
//! used: a box drawn to a different padding than it was sized to is a box whose
//! text overflows it. The comments on the routing weights record what each one
//! was measured to buy, so a later tuning pass knows what it is trading away.

// --- Box interior ----------------------------------------------------------

pub const PADDING: f64 = 28.0;
pub const TITLE_H: f64 = 26.0;
pub const TITLE_GAP: f64 = 14.0;
pub const TOP_PAD: f64 = 12.0;
pub const BOT_PAD: f64 = 14.0;
pub const ICON_SIZE: f64 = 20.0;
pub const ICON_GAP: f64 = 6.0;
pub const TAG_H: f64 = 16.0;
pub const LABEL_H: f64 = 22.0;
pub const TECHN_H: f64 = 16.0;
pub const DESCR_H: f64 = 16.0;
pub const INNER_PAD_X: f64 = 16.0;

// --- Grid ------------------------------------------------------------------

pub const GAP_X: f64 = 56.0;
pub const GAP_Y: f64 = 68.0;
pub const BOX_MIN_W: f64 = 180.0;
pub const BOX_MAX_W: f64 = 300.0;

// --- Type ------------------------------------------------------------------

pub const TAG_FONT: f64 = 11.0;
pub const TAG_WEIGHT: u32 = 600;
pub const LABEL_FONT: f64 = 14.0;
pub const LABEL_WEIGHT: u32 = 600;
pub const TECHN_FONT: f64 = 11.0;
pub const TECHN_WEIGHT: u32 = 400;
pub const DESCR_FONT: f64 = 11.0;
pub const DESCR_WEIGHT: u32 = 400;
pub const DESCR_MAX_LINES: usize = 3;

// --- Relationship badges ---------------------------------------------------

/// Covering a node's name misinforms; overlapping another label is only untidy.
pub const LABEL_BOX_PENALTY: f64 = 6.0;
/// A badge dropped on someone else's wire looks like it belongs to that wire.
pub const LABEL_LINE_PENALTY: f64 = 4.0;
/// How far along a long route the badge sits. Near its source, where the eye can
/// see which box the line leaves; the middle of a long line is both ambiguous
/// and the part most likely to be crossing something.
pub const LABEL_ANCHOR_PX: f64 = 46.0;
pub const BADGE_SIZE: f64 = 22.0;

// --- Edge routing ----------------------------------------------------------

/// The perpendicular stub before the first turn.
pub const EDGE_STUB: f64 = 18.0;
/// How far apart sibling edges sit on a shared box face.
pub const PORT_SPACING: f64 = 26.0;
pub const PORT_MARGIN: f64 = 16.0;
/// Straightness is worth about this many pixels of detour per bend.
pub const TURN_PENALTY: f64 = 600.0;
/// What a second edge pays to share a lane, so parallel runs fan out.
pub const LANE_CONGESTION: f64 = 24.0;
/// What an edge pays to cross another edge rather than run alongside it.
pub const CROSS_PENALTY: f64 = 300.0;
/// What an edge pays for leaving by a face that disagrees with where it is
/// going — off the bottom to reach something above, or off the right to reach
/// something to the left. Such a route reads as a mistake however short it is,
/// so this outweighs a crossing: measured over the five reference diagrams,
/// raising it from 1600 to this drove wrong-facing edges from 9 to 0 for four
/// extra bends and no extra crossings.
pub const FACE_PENALTY: f64 = 14000.0;
/// Multiplier on a lattice step that moves *away* from the target. Distance is
/// otherwise symmetric, so dropping below a box to reach something above it
/// costs a route nothing — and it reads as the line changing its mind.
pub const AWAY_FACTOR: f64 = 5.0;
/// Two runs closer than this read as one line on screen, which is worse than a
/// crossing: a crossing is ambiguous for an instant, a merged run is
/// untraceable for its whole length.
pub const NUDGE_EPS: f64 = 6.0;
/// How far apart separation pulls two runs that were drawn on top of each other.
pub const NUDGE_SEP: f64 = 14.0;

// --- Boundary frames -------------------------------------------------------

pub const BOUNDARY_PAD: f64 = 18.0;
pub const BOUNDARY_LABEL_H: f64 = 22.0;
pub const PERSON_BAR: f64 = 6.0;
