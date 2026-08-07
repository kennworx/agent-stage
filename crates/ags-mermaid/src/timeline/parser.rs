//! Reading `timeline` source.
//!
//! ```text
//! timeline [title <text>]
//! title <text>
//! section <name>
//! <period> : <event> [: <event> …]   a period and its events
//! : <event> [: <event> …]            more events for the period above
//! ```

use super::types::{Period, Section, Timeline};

/// The text after a keyword, when the line opens with it as a whole word.
fn after_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    if !line.get(..keyword.len())?.eq_ignore_ascii_case(keyword) {
        return None;
    }
    let tail = line.get(keyword.len()..)?;
    if tail.chars().next().is_some_and(char::is_alphanumeric) {
        return None;
    }
    Some(tail.trim())
}

/// The event part of a line, split on `:` with blanks dropped.
fn split_events(text: &str) -> Vec<String> {
    text.split(':')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(str::to_string)
        .collect()
}

/// Parse a timeline. A line that matches nothing becomes a bare period, which
/// is what the source most likely meant by it.
pub fn parse(source: &str) -> Timeline {
    let mut timeline = Timeline::default();
    // Index of the section new periods land in, created on demand so a diagram
    // with no `section` directive still has somewhere to put them.
    let mut section: Option<usize> = None;

    for raw in source.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = after_keyword(line, "timeline") {
            if let Some(title) = after_keyword(rest, "title") {
                timeline.title = Some(title.to_string());
            }
            continue;
        }
        if let Some(title) = after_keyword(line, "title") {
            timeline.title = Some(title.to_string());
            continue;
        }
        if let Some(name) = after_keyword(line, "section") {
            timeline.sections.push(Section {
                name: Some(name.to_string()),
                periods: Vec::new(),
            });
            section = Some(timeline.sections.len() - 1);
            continue;
        }

        // A leading colon continues the period above rather than opening one.
        if let Some(rest) = line.strip_prefix(':') {
            let events = split_events(rest);
            if let Some(period) = section
                .and_then(|i| timeline.sections.get_mut(i))
                .and_then(|s| s.periods.last_mut())
            {
                period.events.extend(events);
            }
            continue;
        }

        let (label, events) = match line.split_once(':') {
            Some((label, rest)) => (label.trim(), split_events(rest)),
            None => (line, Vec::new()),
        };
        if label.is_empty() {
            continue;
        }
        let index = *section.get_or_insert_with(|| {
            timeline.sections.push(Section::default());
            timeline.sections.len() - 1
        });
        if let Some(target) = timeline.sections.get_mut(index) {
            target.periods.push(Period {
                label: label.to_string(),
                events,
            });
        }
    }
    timeline
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_period_carries_the_events_on_its_own_line() {
        let t = parse("timeline\n2024 : shipped : reviewed");
        assert_eq!(t.sections.len(), 1);
        assert_eq!(t.sections[0].name, None);
        assert_eq!(t.sections[0].periods[0].label, "2024");
        assert_eq!(t.sections[0].periods[0].events, ["shipped", "reviewed"]);
    }

    #[test]
    fn a_leading_colon_adds_to_the_period_above() {
        let t = parse("timeline\n2024 : first\n: second\n: third : fourth");
        assert_eq!(
            t.sections[0].periods[0].events,
            ["first", "second", "third", "fourth"]
        );
    }

    #[test]
    fn a_continuation_with_no_period_above_it_is_dropped() {
        let t = parse("timeline\n: orphan");
        assert!(t.sections.is_empty());
    }

    #[test]
    fn sections_group_the_periods_that_follow_them() {
        let t = parse("timeline\nsection Alpha\n1 : a\nsection Beta\n2 : b");
        assert_eq!(t.sections.len(), 2);
        assert_eq!(t.sections[0].name.as_deref(), Some("Alpha"));
        assert_eq!(t.sections[1].periods[0].label, "2");
    }

    #[test]
    fn periods_before_any_section_land_in_an_implicit_one() {
        let t = parse("timeline\n1 : a\nsection Named\n2 : b");
        assert_eq!(t.sections.len(), 2);
        assert_eq!(t.sections[0].name, None);
        assert_eq!(t.sections[1].name.as_deref(), Some("Named"));
    }

    #[test]
    fn a_period_may_have_no_events_at_all() {
        let t = parse("timeline\njust a period");
        assert_eq!(t.sections[0].periods[0].label, "just a period");
        assert!(t.sections[0].periods[0].events.is_empty());
    }

    #[test]
    fn a_title_reads_from_the_header_or_its_own_line() {
        assert_eq!(parse("timeline title Ours").title.as_deref(), Some("Ours"));
        assert_eq!(parse("timeline\ntitle Ours").title.as_deref(), Some("Ours"));
    }

    #[test]
    fn a_section_directive_starts_its_periods_afresh() {
        // A continuation after a `section` has nothing above it to extend.
        let t = parse("timeline\n1 : a\nsection New\n: stray");
        assert_eq!(t.sections[0].periods[0].events, ["a"]);
        assert!(t.sections[1].periods.is_empty());
    }

    #[test]
    fn nothing_in_yields_an_empty_timeline() {
        assert_eq!(parse(""), Timeline::default());
        assert_eq!(parse("timeline"), Timeline::default());
    }
}
