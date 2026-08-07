//! Reading `gantt` source.
//!
//! ```text
//! gantt
//!   title <text>
//!   dateFormat YYYY-MM-DD
//!   section <name>
//!   <Task name> : [status,]* [id,] <start>, <duration|end>
//! ```
//!
//! A start is a date or `after <taskId>`; the last field is a duration (`5d`,
//! `2w`, or a bare number of days) or an end date. Everything is resolved to
//! integer day numbers first and shifted to be relative to the earliest date
//! at the end, so a chart with no dates at all still lays out.

use super::types::{Chart, Section, Status, Task};
use crate::keyword::opens_with;

/// Where a task with no section of its own goes.
const DEFAULT_SECTION: &str = "Default";

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

/// Whether `text` is a `YYYY-MM-DD` date.
fn is_date(text: &str) -> bool {
    let parts: Vec<&str> = text.split('-').collect();
    let widths = [4usize, 2, 2];
    parts.len() == 3
        && parts
            .iter()
            .zip(widths)
            .all(|(p, w)| p.len() == w && p.chars().all(|c| c.is_ascii_digit()))
}

/// Days since the epoch for a `YYYY-MM-DD` date.
///
/// Howard Hinnant's civil-days algorithm rather than a date library: the whole
/// of what this needs from dates is a day number and back again.
fn iso_to_day(iso: &str) -> Option<i64> {
    let mut parts = iso.split('-');
    let y: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next()?.parse().ok()?;
    let d: i64 = parts.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// The `YYYY-MM-DD` date a day number names.
fn day_to_iso(day: i64) -> String {
    let z = day + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// `n` days after an ISO date.
pub fn add_days(iso: &str, n: i64) -> String {
    iso_to_day(iso).map_or_else(|| iso.to_string(), |day| day_to_iso(day + n))
}

/// A duration like `5d`, `2w`, `3M`, `1y`, or a bare number of days.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a duration is bounded by what an author writes; the axis is whole days"
)]
fn parse_duration(text: &str) -> i64 {
    let text = text.trim();
    let (number, unit) = match text.char_indices().find(|(_, c)| c.is_ascii_alphabetic()) {
        Some((at, _)) => (
            text.get(..at).unwrap_or_default(),
            text.get(at..).unwrap_or_default().trim(),
        ),
        None => (text, ""),
    };
    let number = number.trim();
    if number.is_empty() || unit.len() > 1 {
        return 0;
    }
    let Ok(n) = number.parse::<f64>() else {
        return 0;
    };
    if !n.is_finite() {
        return 0;
    }
    // Months and years are nominal here — thirty and three hundred and
    // sixty-five days — because a bar's width is days and nothing else.
    let days = match unit {
        "w" | "W" => n * 7.0,
        "M" => n * 30.0,
        "y" | "Y" => n * 365.0,
        "" | "d" | "D" => n,
        _ => return 0,
    };
    // A duration is whole days on the axis; a fractional one truncates rather
    // than rounding, so `1.9d` is a day and not two.
    days.trunc() as i64
}

/// A name as an identifier.
fn slug(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "task".to_string()
    } else {
        out
    }
}

/// An id no other task has claimed.
fn unique_id(base: &str, used: &mut Vec<String>) -> String {
    let mut id = base.to_string();
    let mut n = 2usize;
    while used.contains(&id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    used.push(id.clone());
    id
}

/// One task line, before its start is resolved against the others.
struct Draft {
    task: Task,
    /// An absolute start, when the line named a date.
    absolute: Option<i64>,
    /// The task this one follows, when the line said `after`.
    after: Option<String>,
}

/// Whether a field is a start rather than a duration: a date, or `after X`.
fn is_start_field(field: &str) -> bool {
    is_date(field) || after_keyword(field, "after").is_some()
}

/// Read one `<name> : <fields>` line.
fn parse_task(line: &str, section: &str, used: &mut Vec<String>) -> Option<Draft> {
    let (name, meta) = line.split_once(':')?;
    let (name, meta) = (name.trim(), meta.trim());
    if name.is_empty() || meta.is_empty() {
        return None;
    }
    let fields: Vec<&str> = meta
        .split(',')
        .map(str::trim)
        .filter(|f| !f.is_empty())
        .collect();

    // Leading status keywords, however many of them there are.
    let mut tags = Vec::new();
    let mut rest = fields.as_slice();
    while let Some((first, tail)) = rest.split_first() {
        let Some(status) = Status::from_keyword(first) else {
            break;
        };
        tags.push(status);
        rest = tail;
    }
    if rest.is_empty() {
        return None;
    }

    // What remains is one of `[id, start, end]`, `[start, end]`, `[id, end]`
    // or `[end]`. With two fields the first is a start only if it looks like
    // one, which is what tells an id from a date.
    let (task_id, start_field, end_field) = match rest {
        [id, start, end, ..] => (Some(*id), Some(*start), Some(*end)),
        [first, second] if is_start_field(first) => (None, Some(*first), Some(*second)),
        [first, second] => (Some(*first), None, Some(*second)),
        [end] => (None, None, Some(*end)),
        [] => (None, None, None),
    };

    let mut absolute = None;
    let mut after = None;
    if let Some(field) = start_field {
        if let Some(target) = after_keyword(field, "after") {
            after = target.split_whitespace().next().map(str::to_string);
        } else if is_date(field) {
            absolute = iso_to_day(field);
        }
    }

    let milestone = tags.contains(&Status::Milestone);
    let mut duration = 0i64;
    if let Some(field) = end_field {
        if is_date(field) {
            // An end date is a span, which needs a start to measure from.
            duration = match (absolute, iso_to_day(field)) {
                (Some(start), Some(end)) => (end - start).max(0),
                _ => 1,
            };
        } else {
            duration = parse_duration(field);
        }
    }
    // A milestone is a moment, so it has no width whatever was written.
    if milestone {
        duration = 0;
    }

    let id = unique_id(&task_id.map_or_else(|| slug(name), str::to_string), used);
    Some(Draft {
        task: Task {
            id,
            task_id: task_id.map(str::to_string),
            name: name.to_string(),
            section: section.to_string(),
            tags,
            milestone,
            start_day: 0,
            end_day: 0,
            duration_days: duration,
        },
        absolute,
        after,
    })
}

/// Give every draft an absolute start and end, in source order.
///
/// A task with no start of its own follows the previous one, which is what
/// makes a chart of nothing but durations lay out as a chain.
fn resolve(drafts: &[Draft]) -> Vec<(i64, i64)> {
    let mut spans: Vec<(i64, i64)> = Vec::with_capacity(drafts.len());
    let mut previous_end: Option<i64> = None;
    for draft in drafts {
        let start = if let Some(target) = draft.after.as_deref() {
            drafts
                .iter()
                .position(|d| d.task.task_id.as_deref() == Some(target))
                .and_then(|at| spans.get(at).map(|(_, end)| *end))
                .or(previous_end)
                .unwrap_or(0)
        } else if let Some(absolute) = draft.absolute {
            absolute
        } else {
            previous_end.unwrap_or(0)
        };
        let end = start + draft.task.duration_days;
        spans.push((start, end));
        previous_end = Some(end);
    }
    spans
}

/// Parse a gantt chart. A line that matches nothing is skipped.
pub fn parse(source: &str) -> Chart {
    let mut chart = Chart::default();
    let mut drafts: Vec<Draft> = Vec::new();
    let mut used: Vec<String> = Vec::new();
    let mut section = DEFAULT_SECTION.to_string();

    for raw in source.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() || opens_with(line, "gantt") {
            continue;
        }
        if let Some(format) = after_keyword(line, "dateFormat") {
            chart.date_format = Some(format.to_string());
            continue;
        }
        if let Some(title) = after_keyword(line, "title") {
            chart.title = Some(title.to_string());
            continue;
        }
        if let Some(name) = after_keyword(line, "section") {
            section = name.to_string();
            continue;
        }
        // Directives that change nothing this renderer draws.
        if [
            "axisFormat",
            "excludes",
            "includes",
            "todayMarker",
            "tickInterval",
            "weekday",
        ]
        .iter()
        .any(|k| opens_with(line, k))
        {
            continue;
        }
        if let Some(draft) = parse_task(line, &section, &mut used) {
            drafts.push(draft);
        }
    }

    let spans = resolve(&drafts);
    let earliest = spans.iter().map(|(start, _)| *start).min().unwrap_or(0);
    // The axis shows dates only when at least one task anchored to one.
    if drafts.iter().any(|d| d.absolute.is_some()) {
        chart.start_date = Some(day_to_iso(earliest));
    }

    let mut order: Vec<String> = Vec::new();
    let mut sections: Vec<Section> = Vec::new();
    for (draft, (start, end)) in drafts.into_iter().zip(spans) {
        let mut task = draft.task;
        task.start_day = start - earliest;
        task.end_day = end - earliest;
        if !order.contains(&task.section) {
            order.push(task.section.clone());
            sections.push(Section {
                name: task.section.clone(),
                tasks: Vec::new(),
            });
        }
        if let Some(slot) = sections.iter_mut().find(|s| s.name == task.section) {
            slot.tasks.push(task.clone());
        }
        chart.tasks.push(task);
    }
    chart.sections = sections;
    chart
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHART: &str = "gantt\n\
        title A plan\n\
        dateFormat YYYY-MM-DD\n\
        section Build\n\
        Design      :done, des, 2024-01-01, 5d\n\
        Implement   :active, imp, after des, 10d\n\
        section Ship\n\
        Release     :milestone, rel, after imp, 0d";

    #[test]
    fn a_whole_chart_reads() {
        let chart = parse(CHART);
        assert_eq!(chart.title.as_deref(), Some("A plan"));
        assert_eq!(chart.date_format.as_deref(), Some("YYYY-MM-DD"));
        assert_eq!(chart.sections.len(), 2);
        assert_eq!(chart.tasks.len(), 3);
        assert_eq!(chart.start_date.as_deref(), Some("2024-01-01"));
    }

    #[test]
    fn days_are_counted_from_the_earliest_date() {
        let chart = parse(CHART);
        assert_eq!((chart.tasks[0].start_day, chart.tasks[0].end_day), (0, 5));
        assert_eq!((chart.tasks[1].start_day, chart.tasks[1].end_day), (5, 15));
    }

    #[test]
    fn after_starts_where_the_named_task_ended() {
        assert_eq!(parse(CHART).tasks[2].start_day, 15);
        // And a name that matches nothing falls back to the previous task.
        let chart = parse("gantt\nA :a, 2024-01-01, 2d\nB :after ghost, 1d");
        assert_eq!(chart.tasks[1].start_day, 2);
    }

    #[test]
    fn a_task_with_only_a_duration_follows_the_one_before_it() {
        let chart = parse("gantt\nA :3d\nB :2d\nC :1d");
        let spans: Vec<(i64, i64)> = chart
            .tasks
            .iter()
            .map(|t| (t.start_day, t.end_day))
            .collect();
        assert_eq!(spans, [(0, 3), (3, 5), (5, 6)]);
        // Nothing anchored to a date, so the axis counts days instead.
        assert_eq!(chart.start_date, None);
    }

    #[test]
    fn every_duration_unit_reads() {
        for (text, want) in [
            ("5d", 5),
            ("5D", 5),
            ("2w", 14),
            ("2W", 14),
            ("3M", 90),
            ("1y", 365),
            ("1Y", 365),
            ("7", 7),
            ("1.9d", 1),
        ] {
            assert_eq!(parse_duration(text), want, "{text}");
        }
    }

    #[test]
    fn a_duration_that_is_not_one_is_no_days_at_all() {
        for text in [
            "nonsense",
            "",
            "d",              // a unit with nothing to multiply
            "5q",             // a unit nobody uses
            "5days",          // too long to be a unit
            "1.2.3d",         // not a number
            &"9".repeat(400), // too large to be finite
        ] {
            assert_eq!(parse_duration(text), 0, "{text}");
        }
    }

    #[test]
    fn an_end_date_becomes_a_span_from_the_start() {
        let chart = parse("gantt\nA :2024-01-01, 2024-01-11");
        assert_eq!(chart.tasks[0].duration_days, 10);
    }

    #[test]
    fn a_milestone_has_no_width_whatever_was_written() {
        let chart = parse("gantt\nA :milestone, 2024-01-01, 5d");
        assert!(chart.tasks[0].milestone);
        assert_eq!(chart.tasks[0].duration_days, 0);
    }

    #[test]
    fn a_two_field_line_tells_an_id_from_a_date() {
        // `[start, end]` when the first looks like a start …
        let dated = parse("gantt\nA :2024-01-01, 5d");
        assert_eq!(dated.tasks[0].task_id, None);
        assert_eq!(dated.tasks[0].duration_days, 5);
        // … and `[id, end]` when it does not.
        let named = parse("gantt\nA :myid, 5d");
        assert_eq!(named.tasks[0].task_id.as_deref(), Some("myid"));
        assert_eq!(named.tasks[0].duration_days, 5);
    }

    #[test]
    fn a_task_with_no_id_is_named_after_itself() {
        let chart = parse("gantt\nDesign the thing :3d\nDesign the thing :2d");
        let ids: Vec<&str> = chart.tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["design-the-thing", "design-the-thing-2"]);
    }

    #[test]
    fn a_task_before_any_section_lands_in_a_default_one() {
        let chart = parse("gantt\nOrphan :2d");
        assert_eq!(chart.sections[0].name, "Default");
    }

    #[test]
    fn dates_round_trip_through_day_numbers() {
        for iso in ["1970-01-01", "2024-02-29", "1999-12-31", "2100-03-01"] {
            let day = iso_to_day(iso).expect(iso);
            assert_eq!(day_to_iso(day), iso);
        }
        assert_eq!(add_days("2024-02-28", 2), "2024-03-01", "a leap year");
    }

    #[test]
    fn a_directive_this_renderer_ignores_is_skipped() {
        let chart = parse("gantt\naxisFormat %m-%d\nexcludes weekends\nA :2d");
        assert_eq!(chart.tasks.len(), 1);
    }

    #[test]
    fn nothing_in_yields_an_empty_chart() {
        assert_eq!(parse(""), Chart::default());
        assert_eq!(parse("gantt"), Chart::default());
    }
}
