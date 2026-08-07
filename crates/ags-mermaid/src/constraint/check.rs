//! Asking every rule, in one pass over the scene.

use super::areas::{enclosed_strangers, occluded_labels, outside_canvas};
use super::edges::{backtracking, crossing_edges, edges_through_nodes, merged_edges, wrong_faces};
use super::report::Violation;
use super::scene::{flatten, marked};
use crate::scene::Scene;

/// Check a scene against every legibility rule.
pub fn check(scene: &Scene) -> Vec<Violation> {
    let mut nodes = Vec::new();
    flatten(&scene.nodes, &mut nodes);
    let held = marked(&scene.nodes);
    let mut out = Vec::new();
    out.extend(outside_canvas(&nodes, scene.canvas));
    out.extend(edges_through_nodes(&held));
    out.extend(merged_edges(&held));
    out.extend(occluded_labels(&nodes));
    out.extend(enclosed_strangers(&held));
    out.extend(wrong_faces(&held));
    out.extend(backtracking(&held));
    out.extend(crossing_edges(&held));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constraint::fixture::*;
    use crate::scene::Point;

    #[test]
    fn a_clean_scene_reports_nothing() {
        let mut s = canvas();
        s.push(box_at("a", 10.0, 10.0, 40.0, 20.0));
        s.push(box_at("b", 120.0, 10.0, 40.0, 20.0));
        s.push(wire(
            "e",
            vec![Point::new(50.0, 20.0), Point::new(120.0, 20.0)],
        ));
        assert_eq!(check(&s), vec![]);
    }
}
