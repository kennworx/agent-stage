//! Where the plane sits, and where each component lands on it.
//!
//! The same square as a quadrant chart, read differently: evolution runs left to
//! right and is divided into the four classic stages, visibility runs bottom to
//! top and so inverts against the screen.

use crate::round::count;
use crate::scene::Point;

use super::types::{Kind, Map, Style};

pub const PLOT_SIZE: f64 = 480.0;
pub const PADDING: f64 = 24.0;
pub const TITLE_HEIGHT: f64 = 40.0;
pub const TITLE_FONT: f64 = 18.0;
/// Room below the plot for the stage names and the axis name under them.
pub const X_LABEL_HEIGHT: f64 = 48.0;
pub const Y_LABEL_STRIP: f64 = 30.0;
pub const DOT_RADIUS: f64 = 6.0;
pub const ANCHOR_RADIUS: f64 = 7.0;
pub const LABEL_GAP: f64 = 16.0;

/// The four canonical evolution stages, left to right.
pub const STAGES: [&str; 4] = ["Genesis", "Custom-Built", "Product", "Commodity"];

/// A rectangle, in screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub at: Point,
    pub width: f64,
    pub height: f64,
}

/// A label placed against an axis, and the turn it takes to sit alongside one.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisLabel {
    pub text: String,
    pub at: Point,
    pub rotate: Option<f64>,
}

/// One component, placed.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedComponent {
    /// Unique within the map, so two components sharing a name stay separable.
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub at: Point,
    pub label_at: Point,
}

impl PlacedComponent {
    /// An anchor is drawn a little larger, being the thing everything serves.
    pub const fn radius(&self) -> f64 {
        match self.kind {
            Kind::Anchor => ANCHOR_RADIUS,
            Kind::Component => DOT_RADIUS,
        }
    }
}

/// One dependency, placed centre to centre.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedLink {
    pub from: String,
    pub to: String,
    pub style: Style,
    pub a: Point,
    pub b: Point,
}

/// A laid-out Wardley map.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub width: f64,
    pub height: f64,
    pub title: Option<(String, Point)>,
    pub plot: Option<Rect>,
    /// The three dividers between the four stages.
    pub grid: Vec<(Point, Point)>,
    pub stage_labels: Vec<(String, Point)>,
    pub axis_labels: Vec<AxisLabel>,
    pub links: Vec<PlacedLink>,
    pub components: Vec<PlacedComponent>,
}

/// Where the diagram's name sits: the middle of the band reserved above it.
fn title_baseline() -> f64 {
    f64::midpoint(PADDING, TITLE_HEIGHT)
}

/// A unique id per component, and the id each *name* resolves to.
///
/// A repeated name keeps its first bearer for the purposes of links — a
/// dependency written by name cannot say which of two it meant, so it means the
/// one that was declared first.
fn identify(map: &Map) -> (Vec<String>, Vec<(String, String)>) {
    let mut seen: Vec<(String, usize)> = Vec::new();
    let mut by_name: Vec<(String, String)> = Vec::new();
    let mut ids = Vec::with_capacity(map.components.len());
    for component in &map.components {
        let count = if let Some((_, n)) = seen.iter_mut().find(|(n, _)| *n == component.name) {
            *n += 1;
            *n
        } else {
            seen.push((component.name.clone(), 1));
            1
        };
        let id = if count == 1 {
            component.name.clone()
        } else {
            format!("{}#{count}", component.name)
        };
        if !by_name.iter().any(|(n, _)| *n == component.name) {
            by_name.push((component.name.clone(), id.clone()));
        }
        ids.push(id);
    }
    (ids, by_name)
}

/// Lay out a parsed Wardley map.
pub fn layout(map: &Map) -> Placed {
    let top = PADDING
        + if map.title.is_some() {
            TITLE_HEIGHT
        } else {
            0.0
        };
    // Both strips are always reserved: the axes are named whether or not any
    // component is, so the plot cannot expand into them.
    let left = PADDING + Y_LABEL_STRIP;
    let plot = Rect {
        at: Point::new(left, top),
        width: PLOT_SIZE,
        height: PLOT_SIZE,
    };
    let width = left + PLOT_SIZE + PADDING;
    let height = top + PLOT_SIZE + PADDING + X_LABEL_HEIGHT;

    let x_at = |evolution: f64| plot.at.x + evolution * PLOT_SIZE;
    let y_at = |visibility: f64| plot.at.y + (1.0 - visibility) * PLOT_SIZE;

    let grid = [0.25, 0.5, 0.75]
        .into_iter()
        .map(|e| {
            (
                Point::new(x_at(e), plot.at.y),
                Point::new(x_at(e), plot.at.y + PLOT_SIZE),
            )
        })
        .collect();

    let stage_y = plot.at.y + PLOT_SIZE + 16.0;
    let stage_labels = STAGES
        .into_iter()
        .enumerate()
        .map(|(i, text)| {
            // Centred in its own quarter, not on the divider beside it.
            let at = (count(i) + 0.5) / count(STAGES.len());
            (text.to_string(), Point::new(x_at(at), stage_y))
        })
        .collect();

    let axis_y = plot.at.y + PLOT_SIZE + X_LABEL_HEIGHT - 6.0;
    let axis_labels = vec![
        AxisLabel {
            text: "Evolution".to_string(),
            at: Point::new(plot.at.x + PLOT_SIZE / 2.0, axis_y),
            rotate: None,
        },
        AxisLabel {
            text: "Visibility".to_string(),
            at: Point::new(
                plot.at.x - Y_LABEL_STRIP / 2.0 - 2.0,
                plot.at.y + PLOT_SIZE / 2.0,
            ),
            rotate: Some(-90.0),
        },
    ];

    let (ids, by_name) = identify(map);
    let components: Vec<PlacedComponent> = map
        .components
        .iter()
        .zip(&ids)
        .map(|(component, id)| {
            let at = Point::new(x_at(component.evolution), y_at(component.visibility));
            PlacedComponent {
                id: id.clone(),
                name: component.name.clone(),
                kind: component.kind,
                at,
                label_at: Point::new(at.x, at.y + LABEL_GAP),
            }
        })
        .collect();

    let resolve = |name: &str| {
        by_name
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, id)| components.iter().find(|c| c.id == *id))
    };
    let links = map
        .links
        .iter()
        // A dependency naming a component that was never placed is dropped
        // rather than drawn from nowhere.
        .filter_map(|link| {
            let (a, b) = (resolve(&link.from)?, resolve(&link.to)?);
            Some(PlacedLink {
                from: a.id.clone(),
                to: b.id.clone(),
                style: link.style,
                a: a.at,
                b: b.at,
            })
        })
        .collect();

    Placed {
        width,
        height,
        title: map
            .title
            .clone()
            .map(|text| (text, Point::new(width / 2.0, title_baseline()))),
        plot: Some(plot),
        grid,
        stage_labels,
        axis_labels,
        links,
        components,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wardley::parse;

    fn placed(source: &str) -> Placed {
        layout(&parse(source))
    }

    #[test]
    fn the_plot_is_square_and_the_axis_strips_are_always_reserved() {
        let bare = placed("wardley");
        let plot = bare.plot.expect("a plot");
        assert!((plot.width - PLOT_SIZE).abs() < 1e-9);
        assert!((plot.at.x - (PADDING + Y_LABEL_STRIP)).abs() < 1e-9);
        assert!((bare.height - (PADDING * 2.0 + PLOT_SIZE + X_LABEL_HEIGHT)).abs() < 1e-9);
    }

    #[test]
    fn visibility_inverts_against_the_screen_and_evolution_does_not() {
        let out = placed("wardley\nLow [0, 0]\nHigh [1, 1]");
        let plot = out.plot.expect("a plot");
        // Visibility 0 is the bottom of the plot, the largest screen y.
        assert!((out.components[0].at.y - (plot.at.y + PLOT_SIZE)).abs() < 1e-9);
        assert!((out.components[1].at.y - plot.at.y).abs() < 1e-9);
        // Evolution 0 is the left edge, the smallest screen x.
        assert!((out.components[0].at.x - plot.at.x).abs() < 1e-9);
    }

    #[test]
    fn the_four_stages_are_divided_by_three_lines() {
        let out = placed("wardley");
        assert_eq!(out.grid.len(), 3);
        assert_eq!(out.stage_labels.len(), 4);
        let plot = out.plot.expect("a plot");
        assert!((out.grid[1].0.x - (plot.at.x + PLOT_SIZE / 2.0)).abs() < 1e-9);
    }

    #[test]
    fn a_stage_is_named_in_the_middle_of_its_own_quarter() {
        let out = placed("wardley");
        let plot = out.plot.expect("a plot");
        let (name, at) = &out.stage_labels[0];
        assert_eq!(name, "Genesis");
        // An eighth of the way across, not on the divider at a quarter.
        assert!((at.x - (plot.at.x + PLOT_SIZE * 0.125)).abs() < 1e-9);
    }

    #[test]
    fn an_anchor_is_drawn_larger_than_a_component() {
        let out = placed("wardley\nanchor A [0.9, 0.5]\ncomponent B [0.5, 0.5]");
        assert!(out.components[0].radius() > out.components[1].radius());
    }

    #[test]
    fn a_repeated_name_gets_a_distinct_id_and_links_mean_the_first() {
        let out = placed("wardley\nA [0.9, 0.1]\nA [0.2, 0.8]\nB [0.5, 0.5]\nB -> A");
        let ids: Vec<&str> = out.components.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["A", "A#2", "B"]);
        assert_eq!(out.links[0].to, "A");
        // Which is the one placed high up, not the second one.
        assert!((out.links[0].b.y - out.components[0].at.y).abs() < 1e-9);
    }

    #[test]
    fn a_dependency_on_something_never_placed_is_dropped() {
        assert!(placed("wardley\nA [0.5, 0.5]\nA -> Ghost").links.is_empty());
        assert!(placed("wardley\nA [0.5, 0.5]\nGhost -> A").links.is_empty());
    }

    #[test]
    fn a_dependency_runs_centre_to_centre() {
        let out = placed("wardley\nA [0.9, 0.1]\nB [0.2, 0.8]\nA -> B");
        assert_eq!(out.links[0].a, out.components[0].at);
        assert_eq!(out.links[0].b, out.components[1].at);
    }

    #[test]
    fn a_name_sits_below_its_dot() {
        let out = placed("wardley\nA [0.5, 0.5]");
        let c = &out.components[0];
        assert!((c.label_at.x - c.at.x).abs() < 1e-9);
        assert!((c.label_at.y - c.at.y - LABEL_GAP).abs() < 1e-9);
    }

    #[test]
    fn the_visibility_axis_is_named_turned_and_the_evolution_one_flat() {
        let out = placed("wardley");
        assert_eq!(out.axis_labels[0].text, "Evolution");
        assert_eq!(out.axis_labels[0].rotate, None);
        assert_eq!(out.axis_labels[1].text, "Visibility");
        assert_eq!(out.axis_labels[1].rotate, Some(-90.0));
        assert!(out.axis_labels[1].at.x < out.plot.expect("a plot").at.x);
    }

    #[test]
    fn a_title_pushes_the_plot_down_and_centres_itself() {
        let out = placed("wardley\ntitle T\nA [0.5, 0.5]");
        let (text, at) = out.title.clone().expect("a title");
        assert_eq!(text, "T");
        assert!((at.x - out.width / 2.0).abs() < 1e-9);
        assert!((out.plot.expect("a plot").at.y - (PADDING + TITLE_HEIGHT)).abs() < 1e-9);
    }
}
