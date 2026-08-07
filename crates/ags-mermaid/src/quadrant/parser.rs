//! Reading `quadrantChart` source.
//!
//! ```text
//! quadrantChart
//!   title <text>
//!   x-axis <low> --> <high>       either end may be left unnamed
//!   y-axis <low> --> <high>
//!   quadrant-1 <label>            1 top right, then anticlockwise
//!   <name>: [<x>, <y>]            both in 0..1
//! ```

use super::types::{Axis, Chart, DataPoint};

/// Strip one leading and one trailing quote character, each independently.
fn unquote(text: &str) -> &str {
    let head = text.strip_prefix(['"', '\'']).unwrap_or(text);
    head.strip_suffix(['"', '\'']).unwrap_or(head)
}

/// Everything before a `%%` comment.
fn strip_comment(line: &str) -> &str {
    line.split("%%").next().unwrap_or(line)
}

/// The text after a keyword, when the line opens with it followed by a space.
///
/// The space is required by every directive that carries an argument, so a bare
/// `title` names nothing rather than naming the empty string.
fn after_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    if !line.get(..keyword.len())?.eq_ignore_ascii_case(keyword) {
        return None;
    }
    let tail = line.get(keyword.len()..)?;
    if !tail.starts_with(char::is_whitespace) {
        return None;
    }
    let text = tail.trim();
    (!text.is_empty()).then_some(text)
}

/// A name that is present and not empty.
fn named(text: &str) -> Option<String> {
    let text = unquote(text.trim()).trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// An axis spec split on `-->`. A spec with no arrow names only its low end.
fn split_ends(spec: &str) -> Axis {
    let mut parts = spec.split("-->");
    let low = parts.next().and_then(named);
    // Anything after a second arrow is ignored, as the reference ignores it.
    let high = parts.next().and_then(named);
    Axis { low, high }
}

/// Whether `token` is a number in the form the syntax allows.
///
/// Checked by shape rather than handed straight to a float parser, which would
/// also accept `1e9`, `inf` and `NaN` — none of which the reference reads, and
/// the last two of which would place a point nowhere at all.
fn is_number(token: &str) -> bool {
    let body = token.strip_prefix('-').unwrap_or(token);
    let mut parts = body.splitn(2, '.');
    let whole = parts.next().unwrap_or_default();
    if !whole.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    match parts.next() {
        // `\d*\.\d+` — a dot has to be followed by a digit.
        Some(frac) => !frac.is_empty() && frac.chars().all(|c| c.is_ascii_digit()),
        // `\d+` — with no dot, the whole part carries the digits.
        None => !whole.is_empty(),
    }
}

/// A point that falls outside the square is pulled back to its edge.
fn clamp01(v: f64) -> f64 {
    v.clamp(0.0, 1.0)
}

/// The `[x, y]` half of a point line, if that is all that is left.
fn coordinates(rest: &str) -> Option<(f64, f64)> {
    let inner = rest.trim().strip_prefix('[')?.strip_suffix(']')?;
    let (x, y) = inner.split_once(',')?;
    let (x, y) = (x.trim(), y.trim());
    if !is_number(x) || !is_number(y) {
        return None;
    }
    Some((clamp01(x.parse().ok()?), clamp01(y.parse().ok()?)))
}

/// A `name: [x, y]` line.
///
/// The name runs to the *first* colon that leaves a valid pair behind, so a name
/// may contain a colon as long as the reading stays unambiguous.
fn parse_point(line: &str) -> Option<DataPoint> {
    for (at, _) in line.match_indices(':') {
        let (name, rest) = (line.get(..at)?, line.get(at + 1..)?);
        if let Some((x, y)) = coordinates(rest) {
            return Some(DataPoint {
                name: unquote(name.trim()).trim().to_string(),
                x,
                y,
            });
        }
    }
    None
}

/// A `quadrant-N <label>` line.
fn parse_quadrant(line: &str) -> Option<(u8, String)> {
    const PREFIX: &str = "quadrant-";
    if !line.get(..PREFIX.len())?.eq_ignore_ascii_case(PREFIX) {
        return None;
    }
    let rest = line.get(PREFIX.len()..)?;
    let n = rest.chars().next()?.to_digit(10)?;
    let tail = rest.get(1..)?;
    // The digit has to end there, or `quadrant-12` would name quadrant one.
    if !(1..=4).contains(&n) || !tail.starts_with(char::is_whitespace) {
        return None;
    }
    let label = named(tail)?;
    u8::try_from(n).ok().map(|n| (n, label))
}

/// Parse a quadrant chart. A line that matches nothing is skipped.
pub fn parse(source: &str) -> Chart {
    let mut chart = Chart::default();
    for raw in source.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() || line.eq_ignore_ascii_case("quadrantchart") {
            continue;
        }
        if let Some(title) = after_keyword(line, "title") {
            chart.title = named(title);
            continue;
        }
        if let Some(spec) = after_keyword(line, "x-axis") {
            chart.x_axis = split_ends(spec);
            continue;
        }
        if let Some(spec) = after_keyword(line, "y-axis") {
            chart.y_axis = split_ends(spec);
            continue;
        }
        if let Some((n, label)) = parse_quadrant(line) {
            let slot = match n {
                1 => &mut chart.quadrants.q1,
                2 => &mut chart.quadrants.q2,
                3 => &mut chart.quadrants.q3,
                _ => &mut chart.quadrants.q4,
            };
            *slot = Some(label);
            continue;
        }
        if let Some(point) = parse_point(line) {
            chart.points.push(point);
        }
    }
    chart
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quadrant::Quadrants;

    const CHART: &str = "quadrantChart\n\
        title Reach and engagement\n\
        x-axis Low Reach --> High Reach\n\
        y-axis Low Engagement --> High Engagement\n\
        quadrant-1 We should expand\n\
        quadrant-2 Need promotion\n\
        quadrant-3 Re-evaluate\n\
        quadrant-4 May be improved\n\
        Campaign A: [0.3, 0.6]\n\
        Campaign B: [0.45, 0.23]";

    #[test]
    fn a_whole_chart_reads() {
        let chart = parse(CHART);
        assert_eq!(chart.title.as_deref(), Some("Reach and engagement"));
        assert_eq!(chart.x_axis.low.as_deref(), Some("Low Reach"));
        assert_eq!(chart.x_axis.high.as_deref(), Some("High Reach"));
        assert_eq!(chart.quadrants.q1.as_deref(), Some("We should expand"));
        assert_eq!(chart.quadrants.q4.as_deref(), Some("May be improved"));
        assert_eq!(chart.points.len(), 2);
        assert_eq!(chart.points[0].name, "Campaign A");
    }

    #[test]
    fn an_axis_with_no_arrow_names_only_its_low_end() {
        let axis = parse("quadrantChart\nx-axis Reach").x_axis;
        assert_eq!(axis.low.as_deref(), Some("Reach"));
        assert_eq!(axis.high, None);
    }

    #[test]
    fn an_unnamed_end_is_absent_rather_than_blank() {
        let axis = parse("quadrantChart\nx-axis  --> High").x_axis;
        assert_eq!(axis.low, None);
        assert_eq!(axis.high.as_deref(), Some("High"));
    }

    #[test]
    fn a_point_outside_the_square_is_pulled_back_to_its_edge() {
        let points = parse("quadrantChart\nA: [1.7, -0.4]").points;
        assert!((points[0].x - 1.0).abs() < 1e-9);
        assert!((points[0].y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn a_name_may_hold_a_colon() {
        let points = parse("quadrantChart\nPhase 1: launch: [0.2, 0.8]").points;
        assert_eq!(points[0].name, "Phase 1: launch");
    }

    #[test]
    fn a_coordinate_that_is_not_a_plain_number_is_refused() {
        // Each of these parses as a float but is not what the syntax allows,
        // and two of them would place the point nowhere at all.
        for source in [
            "A: [1e3, 0.5]",
            "A: [inf, 0.5]",
            "A: [NaN, 0.5]",
            "A: [1., 2]",
        ] {
            assert!(
                parse(&format!("quadrantChart\n{source}")).points.is_empty(),
                "{source}"
            );
        }
        assert_eq!(parse("quadrantChart\nA: [.5, -.5]").points.len(), 1);
    }

    #[test]
    fn a_malformed_point_is_skipped_rather_than_fatal() {
        for source in [
            "A [0.1, 0.2]",
            "A: 0.1, 0.2",
            "A: [0.1]",
            "A: [0.1, 0.2",
            "A: [a, b]",
        ] {
            assert!(
                parse(&format!("quadrantChart\n{source}")).points.is_empty(),
                "{source}"
            );
        }
    }

    #[test]
    fn a_quadrant_number_outside_one_to_four_names_nothing() {
        let quads = parse("quadrantChart\nquadrant-5 Nowhere\nquadrant-0 Also nowhere").quadrants;
        assert_eq!(quads, Quadrants::default());
        // And a digit that runs on is not quadrant one.
        assert_eq!(
            parse("quadrantChart\nquadrant-12 Twelve").quadrants.q1,
            None
        );
    }

    #[test]
    fn quotes_around_a_name_are_optional() {
        let chart = parse("quadrantChart\ntitle \"Quoted\"\n\"Point\": [0.1, 0.2]");
        assert_eq!(chart.title.as_deref(), Some("Quoted"));
        assert_eq!(chart.points[0].name, "Point");
    }

    #[test]
    fn a_directive_with_no_argument_names_nothing() {
        assert_eq!(parse("quadrantChart\ntitle").title, None);
        assert_eq!(parse("quadrantChart\nx-axis").x_axis, Axis::default());
        assert_eq!(parse("quadrantChart\nquadrant-1").quadrants.q1, None);
    }

    #[test]
    fn a_comment_is_stripped_before_the_line_is_read() {
        assert_eq!(
            parse("quadrantChart\ntitle Shown %% hidden")
                .title
                .as_deref(),
            Some("Shown")
        );
    }

    #[test]
    fn nothing_in_yields_an_empty_chart() {
        assert_eq!(parse(""), Chart::default());
        assert_eq!(parse("quadrantChart"), Chart::default());
    }
}
