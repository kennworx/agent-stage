//! Reading `radar-beta` source.
//!
//! ```text
//! radar-beta [title <text>]
//!   title <text>
//!   max <number>                       the radial scale's upper bound
//!   axis a["Label"], b, c              named spokes; the label is optional
//!   curve x["Label"]{1, 2, 3}          values in axis order
//!   curve y["Label"]{ c: 3, a: 1 }     values by axis id, in any order
//! ```
//!
//! `showLegend`, `graticule` and `ticks` are accepted and ignored: they are
//! cosmetic in the reference and change nothing that is drawn here.

use super::types::{Axis, Chart, Series};
use crate::keyword::is_word;

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

/// Whether `token` is a number in the form the syntax allows.
fn is_number(token: &str) -> bool {
    let body = token.strip_prefix('-').unwrap_or(token);
    let mut parts = body.splitn(2, '.');
    let whole = parts.next().unwrap_or_default();
    if !whole.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    match parts.next() {
        Some(frac) => !frac.is_empty() && frac.chars().all(|c| c.is_ascii_digit()),
        None => !whole.is_empty(),
    }
}

/// Read a number in the form the syntax allows, or nothing.
fn number(token: &str) -> Option<f64> {
    let token = token.trim();
    is_number(token).then(|| token.parse().ok())?
}

/// Split on commas that are not inside brackets or braces.
fn split_top_level(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for c in text.chars() {
        match c {
            '[' | '{' | '(' => depth += 1,
            ']' | '}' | ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if c == ',' && depth == 0 {
            if !current.trim().is_empty() {
                out.push(current.trim().to_string());
            }
            current.clear();
        } else {
            current.push(c);
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

/// An identifier, and the `[Label]` after it if there is one.
fn id_and_label(text: &str) -> Option<(String, String)> {
    let text = text.trim();
    let mut chars = text.char_indices();
    let end = loop {
        match chars.next() {
            Some((i, c)) if !is_word(c) => break i,
            Some(_) => {}
            None => break text.len(),
        }
    };
    let id = text.get(..end)?;
    if id.is_empty() {
        return None;
    }
    let rest = text.get(end..)?.trim();
    if rest.is_empty() {
        return Some((id.to_string(), id.to_string()));
    }
    let inner = rest.strip_prefix('[')?.strip_suffix(']')?.trim();
    let label = if inner.is_empty() {
        id.to_string()
    } else {
        unquote(inner).to_string()
    };
    Some((id.to_string(), label))
}

/// A `curve id["Label"]{…}` line.
fn parse_curve(line: &str, axes: &[Axis]) -> Option<Series> {
    let rest = after_keyword(line, "curve")?;
    let body_start = rest.find('{')?;
    let head = rest.get(..body_start)?;
    let body = rest.get(body_start..)?.trim();
    let body = body.strip_prefix('{')?.strip_suffix('}')?.trim();
    let (id, label) = id_and_label(head)?;

    let mut values = vec![0.0; axes.len()];
    if body.contains(':') {
        // Named form: each pair says which axis it is for, so order is free.
        for pair in split_top_level(body) {
            let Some((name, value)) = pair.split_once(':') else {
                continue;
            };
            let Some(value) = number(value) else { continue };
            if let Some(index) = axes.iter().position(|a| a.id == name.trim()) {
                if let Some(slot) = values.get_mut(index) {
                    *slot = value;
                }
            }
        }
    } else {
        let ordered: Vec<f64> = split_top_level(body)
            .iter()
            // A value that will not read is zero rather than a hole, so the
            // polygon still closes.
            .map(|token| number(token).unwrap_or(0.0))
            .collect();
        // With no axes declared yet, the value list is what sets the length.
        if axes.is_empty() {
            return Some(Series {
                id,
                label,
                values: ordered,
            });
        }
        for (i, value) in ordered.into_iter().enumerate() {
            if let Some(slot) = values.get_mut(i) {
                *slot = value;
            }
        }
    }
    Some(Series { id, label, values })
}

/// Make every id in a list unique, suffixing whichever repeats.
fn dedupe(ids: &mut [&mut String]) {
    let mut seen: Vec<(String, usize)> = Vec::new();
    for id in ids {
        let count = seen
            .iter()
            .find(|(name, _)| name == *id)
            .map_or(0, |(_, n)| *n);
        if count > 0 {
            **id = format!("{id}_{count}");
        }
        // Keyed on the name before any suffix, so a third repeat is numbered
        // from the same tally as the second.
        let base = id.rsplit_once('_').map_or(id.as_str(), |(head, tail)| {
            if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
                head
            } else {
                id.as_str()
            }
        });
        let base = base.to_string();
        if let Some(slot) = seen.iter_mut().find(|(name, _)| *name == base) {
            slot.1 = count + 1;
        } else {
            seen.push((base, count + 1));
        }
    }
}

/// Parse a radar chart. A line that matches nothing is skipped.
pub fn parse(source: &str) -> Chart {
    let mut chart = Chart::default();
    for raw in source.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        // The header may carry the title on the same line.
        let header = line
            .get(..10)
            .filter(|h| h.eq_ignore_ascii_case("radar-beta"))
            .and_then(|_| line.get(10..))
            .or_else(|| {
                line.get(..5)
                    .filter(|h| h.eq_ignore_ascii_case("radar"))
                    .and_then(|_| line.get(5..))
            })
            .filter(|rest| rest.chars().next().is_none_or(|c| !is_word(c)));
        if let Some(rest) = header {
            if let Some(title) = after_keyword(rest.trim(), "title") {
                chart.title = Some(unquote(title).to_string());
            }
            continue;
        }
        if let Some(title) = after_keyword(line, "title") {
            chart.title = Some(unquote(title).to_string());
            continue;
        }
        if let Some(rest) = after_keyword(line, "max") {
            // A scale of zero or less would divide every value by nothing.
            if let Some(max) = number(rest).filter(|m| *m > 0.0) {
                chart.max = Some(max);
            }
            continue;
        }
        if let Some(rest) = after_keyword(line, "axis") {
            for def in split_top_level(rest) {
                if let Some((id, label)) = id_and_label(&def) {
                    chart.axes.push(Axis { id, label });
                }
            }
            continue;
        }
        if let Some(series) = parse_curve(line, &chart.axes) {
            chart.series.push(series);
        }
    }
    let mut axis_ids: Vec<&mut String> = chart.axes.iter_mut().map(|a| &mut a.id).collect();
    dedupe(&mut axis_ids);
    let mut series_ids: Vec<&mut String> = chart.series.iter_mut().map(|s| &mut s.id).collect();
    dedupe(&mut series_ids);
    chart
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHART: &str = "radar-beta\n\
        title Skills\n\
        axis code[\"Coding\"], design, ops\n\
        curve now[\"Today\"]{4, 2, 3}\n\
        curve goal[\"Target\"]{ ops: 5, code: 5 }";

    #[test]
    fn a_whole_chart_reads() {
        let chart = parse(CHART);
        assert_eq!(chart.title.as_deref(), Some("Skills"));
        assert_eq!(chart.axes.len(), 3);
        assert_eq!(chart.axes[0].label, "Coding");
        // An axis with no label is named by its own id.
        assert_eq!(chart.axes[1].label, "design");
        assert_eq!(chart.series.len(), 2);
    }

    #[test]
    fn ordered_values_follow_axis_order() {
        assert_eq!(parse(CHART).series[0].values, [4.0, 2.0, 3.0]);
    }

    #[test]
    fn named_values_may_come_in_any_order_and_fill_the_rest_with_zero() {
        assert_eq!(parse(CHART).series[1].values, [5.0, 0.0, 5.0]);
    }

    #[test]
    fn a_title_may_ride_on_the_header_line() {
        assert_eq!(
            parse("radar-beta title On the header").title.as_deref(),
            Some("On the header")
        );
        assert_eq!(
            parse("radar title Short form").title.as_deref(),
            Some("Short form")
        );
    }

    #[test]
    fn a_scale_that_would_divide_by_nothing_is_refused() {
        assert_eq!(parse("radar\nmax 100").max, Some(100.0));
        assert_eq!(parse("radar\nmax 0").max, None);
        assert_eq!(parse("radar\nmax -5").max, None);
    }

    #[test]
    fn a_curve_written_before_any_axis_takes_its_length_from_its_values() {
        let chart = parse("radar\ncurve a{1, 2, 3, 4}");
        assert_eq!(chart.series[0].values.len(), 4);
    }

    #[test]
    fn a_value_that_will_not_read_is_zero_rather_than_a_hole() {
        // The polygon still has to close, so a bad value cannot drop a vertex.
        let chart = parse("radar\naxis a, b, c\ncurve x{1, oops, 3}");
        assert_eq!(chart.series[0].values, [1.0, 0.0, 3.0]);
    }

    #[test]
    fn a_named_value_for_an_axis_that_does_not_exist_is_ignored() {
        let chart = parse("radar\naxis a, b\ncurve x{ a: 1, ghost: 9 }");
        assert_eq!(chart.series[0].values, [1.0, 0.0]);
    }

    #[test]
    fn repeated_ids_are_made_unique() {
        let chart = parse("radar\naxis a, a, a\ncurve x{1}\ncurve x{2}");
        let axes: Vec<&str> = chart.axes.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(axes, ["a", "a_1", "a_2"]);
        let series: Vec<&str> = chart.series.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(series, ["x", "x_1"]);
    }

    #[test]
    fn a_cosmetic_directive_is_accepted_and_changes_nothing() {
        let plain = parse("radar\naxis a, b\ncurve x{1, 2}");
        let decorated =
            parse("radar\nshowLegend true\ngraticule polygon\naxis a, b\ncurve x{1, 2}");
        assert_eq!(plain.axes, decorated.axes);
        assert_eq!(plain.series, decorated.series);
    }

    #[test]
    fn nothing_in_yields_an_empty_chart() {
        assert_eq!(parse(""), Chart::default());
        assert_eq!(parse("radar-beta"), Chart::default());
    }
}
