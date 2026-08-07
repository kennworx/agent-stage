//! Reading `xychart-beta` source.
//!
//! ```text
//! xychart-beta [horizontal]
//!   title "Product Sales"
//!   x-axis [Widgets, Gadgets, Gizmos]      x-axis "Month" [Jan, Feb]
//!   x-axis 0 --> 100                        y-axis "Users" 0 --> 30000
//!   bar [150, 230, 180]
//!   line [4, 7, 13]
//! ```
//!
//! Only the y axis gets a span derived for it when the source declares none —
//! the data's own extent, padded by a tenth, and floored to zero when the values
//! sit near it. The x axis is left as it was written, because a bar chart's x
//! axis is a list of names far more often than it is a number line.
//!
//! Matching is hand-rolled rather than regex-driven, as everywhere else in this
//! crate — see `text.rs` for why.

use super::types::{Axis, Chart, Range, Series, SeriesKind};
use crate::keyword::{is_word, opens_with};

/// How much of the data's own span is added above and below it.
const PAD_RATIO: f64 = 0.1;
/// A floor of zero is used when the smallest value is within this much of the
/// span from it — a bar chart that does not start at zero misleads.
const ZERO_FLOOR_RATIO: f64 = 0.5;

/// Whether `haystack` contains `word` as a whole word, ignoring case.
fn has_word(haystack: &str, word: &str) -> bool {
    haystack.char_indices().any(|(at, _)| {
        let after_a_word = haystack
            .get(..at)
            .and_then(|before| before.chars().next_back())
            .is_some_and(is_word);
        !after_a_word
            && haystack
                .get(at..)
                .is_some_and(|rest| opens_with(rest, word))
    })
}

/// The text after `word` and the whitespace that must follow it.
fn after_word<'a>(line: &'a str, word: &str) -> Option<&'a str> {
    if line.get(..word.len())? != word {
        return None;
    }
    let rest = line.get(word.len()..)?;
    rest.starts_with(char::is_whitespace)
        .then(|| rest.trim_start())
}

/// The contents of a leading `"…"`, and what follows it.
fn quoted(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some((rest.get(..end)?, rest.get(end + 1..)?))
}

/// The contents of a leading `[…]`.
fn bracketed(text: &str) -> Option<&str> {
    let rest = text.strip_prefix('[')?;
    let end = rest.find(']')?;
    // The reference's `[^\]]+` needs at least one character between the pair.
    rest.get(..end).filter(|inner| !inner.is_empty())
}

/// A signed decimal at the head of `text`, and what follows it.
///
/// Deliberately narrower than Rust's own parser: the reference's
/// `-?\d+(?:\.\d+)?` accepts no exponent, no leading dot and no `+`, so neither
/// does this.
fn decimal(text: &str) -> Option<(f64, &str)> {
    let body = text.strip_prefix('-').unwrap_or(text);
    let digits = body
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(body.len());
    if digits == 0 {
        return None;
    }
    let mut end = text.len() - body.len() + digits;
    if let Some(fraction) = text.get(end..).and_then(|rest| rest.strip_prefix('.')) {
        let places = fraction
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(fraction.len());
        if places > 0 {
            end += 1 + places;
        }
    }
    let number = text.get(..end)?.parse::<f64>().ok()?;
    Some((number, text.get(end..)?))
}

/// `<min> --> <max>`.
fn span(text: &str) -> Option<Range> {
    let (min, rest) = decimal(text)?;
    let (max, _) = decimal(rest.trim_start().strip_prefix("-->")?.trim_start())?;
    Some(Range { min, max })
}

/// The optional `"Title"` in front of an axis declaration, and the rest.
fn axis_title(text: &str) -> (Option<String>, &str) {
    match quoted(text) {
        // An empty title is no title: the reference's `if (match[1])` drops it.
        Some((title, rest)) if !title.is_empty() => (Some(title.to_string()), rest.trim_start()),
        Some((_, rest)) => (None, rest.trim_start()),
        None => (None, text),
    }
}

/// Read one axis declaration into `axis`. Answers whether it was one.
fn read_axis(axis: &mut Axis, tail: &str, allow_categories: bool) -> bool {
    let (title, rest) = axis_title(tail);
    // A title is only recorded when this line turned out to be an axis, and
    // never clears one an earlier line set.
    if allow_categories {
        if let Some(inner) = bracketed(rest) {
            axis.title = title.or_else(|| axis.title.clone());
            axis.categories = Some(inner.split(',').map(|c| c.trim().to_string()).collect());
            return true;
        }
    }
    if let Some(range) = span(rest) {
        axis.title = title.or_else(|| axis.title.clone());
        axis.range = Some(range);
        return true;
    }
    false
}

/// The numbers in a `[…]` list.
///
/// A value that is not a number reads as zero. The reference produces `NaN`,
/// which then spreads through the scale and puts `NaN` into every coordinate on
/// the page; a zero leaves the rest of the chart drawable.
fn values(inner: &str) -> Vec<f64> {
    inner
        .split(',')
        .map(|value| value.trim().parse::<f64>().unwrap_or(0.0))
        .collect()
}

/// The span the values themselves ask for, padded so nothing touches the edge.
fn derived_range(series: &[Series]) -> Option<Range> {
    let mut values = series.iter().flat_map(|s| s.data.iter().copied());
    let first = values.next()?;
    let (mut min, mut max) = values.fold((first, first), |(lo, hi), v| (lo.min(v), hi.max(v)));
    // A flat series has no span of its own, so give it one to divide by.
    let range = if max - min == 0.0 { 1.0 } else { max - min };
    min -= range * PAD_RATIO;
    max += range * PAD_RATIO;
    if min > 0.0 && min < range * ZERO_FLOOR_RATIO {
        min = 0.0;
    }
    Some(Range { min, max })
}

/// Read one line into the chart being built.
fn read_line(chart: &mut Chart, line: &str) {
    // `xychart-beta` is caught by this too: `-` ends the word either way.
    if opens_with(line, "xychart") {
        chart.horizontal |= has_word(line, "horizontal");
        return;
    }
    if let Some(rest) = after_word(line, "title") {
        if let Some((title, _)) = quoted(rest) {
            if !title.is_empty() {
                chart.title = Some(title.to_string());
            }
        }
        return;
    }
    if let Some(rest) = after_word(line, "x-axis") {
        read_axis(&mut chart.x_axis, rest, true);
        return;
    }
    if let Some(rest) = after_word(line, "y-axis") {
        // A y axis takes a span or a bare title, but never a list of names.
        if !read_axis(&mut chart.y_axis, rest, false) {
            if let Some((title, after)) = quoted(rest) {
                if !title.is_empty() && after.trim().is_empty() {
                    chart.y_axis.title = Some(title.to_string());
                }
            }
        }
        return;
    }
    for (word, kind) in [("bar", SeriesKind::Bar), ("line", SeriesKind::Line)] {
        if let Some(rest) = after_word(line, word) {
            if let Some(inner) = bracketed(rest) {
                chart.series.push(Series {
                    kind,
                    data: values(inner),
                });
            }
            return;
        }
    }
}

/// Parse an xy chart source.
pub fn parse(source: &str) -> Chart {
    let mut chart = Chart::default();
    for line in source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("%%"))
    {
        read_line(&mut chart, line);
    }
    if chart.y_axis.range.is_none() {
        chart.y_axis.range = derived_range(&chart.series);
    }
    chart.y_axis.range.get_or_insert(Range {
        min: 0.0,
        max: 100.0,
    });
    chart
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(chart: &Chart) -> Range {
        chart.y_axis.range.expect("a derived span")
    }

    #[test]
    fn a_header_declares_the_chart_and_its_direction() {
        assert!(!parse("xychart-beta\nbar [1]").horizontal);
        assert!(parse("xychart-beta horizontal\nbar [1]").horizontal);
        assert!(parse("xychart horizontal\nbar [1]").horizontal);
        assert!(parse("XYChart-Beta HORIZONTAL\nbar [1]").horizontal);
    }

    #[test]
    fn a_word_that_merely_contains_horizontal_does_not_turn_the_chart() {
        assert!(!parse("xychart-beta horizontally\nbar [1]").horizontal);
    }

    #[test]
    fn a_title_is_read_from_its_quotes() {
        assert_eq!(
            parse("xychart-beta\ntitle \"Product Sales\"\nbar [1]")
                .title
                .as_deref(),
            Some("Product Sales")
        );
        assert_eq!(parse("xychart-beta\ntitle Sales\nbar [1]").title, None);
        assert_eq!(parse("xychart-beta\ntitle \"\"\nbar [1]").title, None);
    }

    #[test]
    fn an_x_axis_reads_a_list_of_names() {
        let out = parse("xychart-beta\nx-axis [Widgets, Gadgets, Gizmos]\nbar [1,2,3]");
        assert_eq!(
            out.x_axis.categories.as_deref(),
            Some(["Widgets".to_string(), "Gadgets".into(), "Gizmos".into()].as_slice())
        );
        assert_eq!(out.x_axis.title, None);
    }

    #[test]
    fn an_x_axis_reads_a_title_in_front_of_its_names() {
        let out = parse("xychart-beta\nx-axis \"Month\" [Jan, Feb]\nbar [1,2]");
        assert_eq!(out.x_axis.title.as_deref(), Some("Month"));
        assert_eq!(out.x_axis.categories.as_ref().map(Vec::len), Some(2));
    }

    #[test]
    fn an_empty_title_in_front_of_an_axis_is_no_title_at_all() {
        let out = parse("xychart-beta\nx-axis \"\" [Jan, Feb]\nbar [1,2]");
        assert_eq!(out.x_axis.title, None);
        assert_eq!(out.x_axis.categories.as_ref().map(Vec::len), Some(2));
    }

    #[test]
    fn an_axis_reads_a_numeric_span() {
        let out =
            parse("xychart-beta\nx-axis 0 --> 100\ny-axis \"Users\" -5.5 --> 30000\nline [1]");
        assert_eq!(
            out.x_axis.range,
            Some(Range {
                min: 0.0,
                max: 100.0
            })
        );
        assert_eq!(
            out.y_axis.range,
            Some(Range {
                min: -5.5,
                max: 30000.0
            })
        );
        assert_eq!(out.y_axis.title.as_deref(), Some("Users"));
    }

    #[test]
    fn a_y_axis_may_carry_only_a_title() {
        let out = parse("xychart-beta\ny-axis \"Story Points\"\nbar [10, 20]");
        assert_eq!(out.y_axis.title.as_deref(), Some("Story Points"));
        // The span is still derived from the data.
        assert!(range(&out).max > 20.0);
    }

    #[test]
    fn a_y_axis_never_reads_a_list_of_names() {
        let out = parse("xychart-beta\ny-axis [a, b]\nbar [1, 2]");
        assert_eq!(out.y_axis.categories, None);
        assert_eq!(out.y_axis.title, None);
    }

    #[test]
    fn a_span_needs_both_ends_and_an_arrow() {
        for line in [
            "x-axis 0 100",
            "x-axis 0 -->",
            "x-axis --> 100",
            "x-axis a --> b",
        ] {
            let out = parse(&format!("xychart-beta\n{line}\nbar [1]"));
            assert_eq!(out.x_axis.range, None, "{line}");
        }
    }

    #[test]
    fn a_number_is_read_the_way_the_reference_reads_one() {
        // No exponent, no leading dot, no plus sign.
        assert_eq!(decimal("12.5rest").map(|(v, _)| v), Some(12.5));
        assert_eq!(decimal("-7 rest").map(|(v, _)| v), Some(-7.0));
        assert_eq!(decimal("3.").map(|(v, _)| v), Some(3.0));
        assert_eq!(decimal(".5"), None);
        assert_eq!(decimal("+5"), None);
        assert_eq!(decimal("e5"), None);
        assert_eq!(decimal("1e3"), Some((1.0, "e3")));
    }

    #[test]
    fn every_series_keyword_is_read_in_source_order() {
        let out = parse("xychart-beta\nbar [1, 2]\nline [3, 4]\nbar [5, 6]");
        let kinds: Vec<SeriesKind> = out.series.iter().map(|s| s.kind).collect();
        assert_eq!(kinds, [SeriesKind::Bar, SeriesKind::Line, SeriesKind::Bar]);
        assert_eq!(out.series[1].data, [3.0, 4.0]);
    }

    #[test]
    fn a_series_with_no_brackets_is_not_one() {
        let out = parse("xychart-beta\nbar 1, 2\nline []\nbar [3]");
        assert_eq!(out.series.len(), 1);
        assert_eq!(out.series[0].data, [3.0]);
    }

    #[test]
    fn a_value_that_is_not_a_number_reads_as_zero() {
        // The reference yields NaN here, which then reaches every coordinate on
        // the page; a zero keeps the rest of the chart drawable.
        assert_eq!(
            parse("xychart-beta\nbar [1, oops, 3]").series[0].data,
            [1.0, 0.0, 3.0]
        );
    }

    #[test]
    fn a_span_is_derived_from_the_data_with_room_above_and_below() {
        let out = parse("xychart-beta\nline [100, 200]");
        // A span of 100, a tenth either side, and the floor is far from zero.
        assert_eq!(
            range(&out),
            Range {
                min: 90.0,
                max: 210.0
            }
        );
    }

    #[test]
    fn a_span_that_nearly_reaches_zero_is_taken_down_to_it() {
        let out = parse("xychart-beta\nbar [10, 100]");
        assert!((range(&out).min - 0.0).abs() < 1e-9);
        assert!((range(&out).max - 109.0).abs() < 1e-9);
    }

    #[test]
    fn a_flat_series_still_gets_a_span_to_divide_by() {
        let out = parse("xychart-beta\nbar [50, 50]");
        assert!(range(&out).max > range(&out).min);
    }

    #[test]
    fn a_declared_span_is_never_overwritten_by_the_data() {
        let out = parse("xychart-beta\ny-axis 0 --> 10000\nbar [4200, 9200]");
        assert_eq!(
            range(&out),
            Range {
                min: 0.0,
                max: 10000.0
            }
        );
    }

    #[test]
    fn a_chart_with_no_data_at_all_still_gets_a_span() {
        assert_eq!(
            range(&parse("xychart-beta")),
            Range {
                min: 0.0,
                max: 100.0
            }
        );
        assert_eq!(parse("xychart-beta").series.len(), 0);
    }

    #[test]
    fn a_comment_and_a_blank_line_are_dropped_before_reading() {
        let out = parse("xychart-beta\n\n%% a note\n  bar [1, 2]  \n");
        assert_eq!(out.series.len(), 1);
    }

    #[test]
    fn a_line_that_names_nothing_is_ignored() {
        let out = parse("xychart-beta\nwhat is this\nbar [1]");
        assert_eq!(out.series.len(), 1);
    }
}
