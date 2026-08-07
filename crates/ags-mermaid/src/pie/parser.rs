//! Reading `pie` source.
//!
//! Three line shapes, and nothing else:
//!
//! ```text
//! pie [showData] [title <text>]
//! title <text>
//! "Label" : <value>
//! ```

use super::types::{Chart, Slice};

/// Strip one leading and one trailing quote character.
///
/// Each end independently, as the reference does — so a label written with only
/// an opening quote still loses it, rather than keeping a stray character
/// because its partner was missing.
fn unquote(text: &str) -> &str {
    let trimmed = text.trim();
    let head = trimmed.strip_prefix(['"', '\'']).unwrap_or(trimmed);
    head.strip_suffix(['"', '\'']).unwrap_or(head)
}

/// The text after a keyword, when the line opens with it.
fn after_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.get(..keyword.len())?;
    if !rest.eq_ignore_ascii_case(keyword) {
        return None;
    }
    let tail = line.get(keyword.len()..)?;
    // The keyword has to be a whole word, or `titled` would open a title.
    if tail.chars().next().is_some_and(char::is_alphanumeric) {
        return None;
    }
    Some(tail.trim())
}

/// One `"Label" : value` line, or nothing when the line is not one.
fn parse_slice(line: &str) -> Option<Slice> {
    let (label, value) = line.rsplit_once(':')?;
    let label = unquote(label).trim().to_string();
    let value: f64 = value.trim().parse().ok()?;
    if label.is_empty() || !value.is_finite() || value < 0.0 {
        return None;
    }
    Some(Slice { label, value })
}

/// Parse a pie chart. A line that matches nothing is skipped: a diagram beats
/// an error over one stray line.
pub fn parse(source: &str) -> Chart {
    let mut chart = Chart::default();
    for raw in source.lines() {
        let line = raw.trim();
        if let Some(rest) = after_keyword(line, "pie") {
            let rest = match after_keyword(rest, "showData") {
                Some(after) => {
                    chart.show_data = true;
                    after
                }
                None => rest,
            };
            if let Some(title) = after_keyword(rest, "title") {
                chart.title = Some(unquote(title).to_string());
            }
            continue;
        }
        if let Some(title) = after_keyword(line, "title") {
            chart.title = Some(unquote(title).to_string());
            continue;
        }
        if let Some(slice) = parse_slice(line) {
            chart.slices.push(slice);
        }
    }
    chart
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_can_carry_the_title_and_the_data_flag() {
        let chart = parse("pie showData title Shares\n\"a\" : 1");
        assert_eq!(chart.title.as_deref(), Some("Shares"));
        assert!(chart.show_data);
        assert_eq!(chart.slices.len(), 1);
    }

    #[test]
    fn a_title_can_stand_on_its_own_line() {
        let chart = parse("pie\ntitle Shares of the thing\n\"a\" : 1");
        assert_eq!(chart.title.as_deref(), Some("Shares of the thing"));
        assert!(!chart.show_data);
    }

    #[test]
    fn a_quoted_title_loses_its_quotes() {
        assert_eq!(
            parse("pie title \"Quoted\"").title.as_deref(),
            Some("Quoted")
        );
        assert_eq!(
            parse("pie\ntitle 'Single'").title.as_deref(),
            Some("Single")
        );
    }

    #[test]
    fn slices_take_their_label_and_value() {
        let chart = parse("pie\n\"Rust\" : 40\n\"Go\" : 25.5\nPlain : 10");
        assert_eq!(
            chart.slices,
            vec![
                Slice {
                    label: "Rust".into(),
                    value: 40.0
                },
                Slice {
                    label: "Go".into(),
                    value: 25.5
                },
                Slice {
                    label: "Plain".into(),
                    value: 10.0
                },
            ]
        );
    }

    #[test]
    fn a_label_may_itself_contain_a_colon() {
        // Split from the right, so the value is the last field.
        let chart = parse("pie\n\"ratio 3:1\" : 7");
        assert_eq!(chart.slices[0].label, "ratio 3:1");
        assert!((chart.slices[0].value - 7.0).abs() < 1e-9);
    }

    #[test]
    fn a_line_that_is_not_a_slice_is_skipped_rather_than_fatal() {
        let chart = parse("pie\nnonsense\n\"a\" : 1\n%% a comment\n\"b\" : x");
        assert_eq!(chart.slices.len(), 1);
    }

    #[test]
    fn a_negative_or_unlabelled_slice_is_refused() {
        assert!(parse("pie\n\"a\" : -1").slices.is_empty());
        assert!(parse("pie\n\"\" : 5").slices.is_empty());
    }

    #[test]
    fn the_keyword_has_to_be_a_whole_word() {
        // `titled` is not `title`, and would otherwise eat the line.
        let chart = parse("pie\ntitled thing : 4");
        assert!(chart.title.is_none());
        assert_eq!(chart.slices.len(), 1);
    }

    #[test]
    fn nothing_in_yields_an_empty_chart() {
        assert_eq!(parse(""), Chart::default());
        assert_eq!(parse("pie"), Chart::default());
    }
}
