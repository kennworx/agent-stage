//! Reading `journey` source.
//!
//! ```text
//! journey
//!   title <text>
//!   section <name>
//!   <Task> : <score 1-5> : <Actor>, <Actor>
//! ```
//!
//! A task written before any `section` still belongs somewhere, so it lands in
//! an implicit unnamed one rather than being dropped.

use super::types::{Journey, Section, Task};
use crate::keyword::opens_with;

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

/// Scores outside the scale are pulled onto it rather than off the chart.
fn clamp_score(score: i32) -> i32 {
    score.clamp(1, 5)
}

/// A `Task : score : Actor, Actor` row.
fn parse_task(line: &str) -> Option<Task> {
    let (name, rest) = line.split_once(':')?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    // The score is its own field; everything past the next colon is actors,
    // colons and all, so an actor's name may contain one.
    let (score, actors) = match rest.split_once(':') {
        Some((score, actors)) => (score, actors.trim()),
        None => (rest, ""),
    };
    let score: i32 = score.trim().parse().ok()?;
    Some(Task {
        name: name.to_string(),
        score: clamp_score(score),
        actors: actors
            .split(',')
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .map(str::to_string)
            .collect(),
    })
}

/// Parse a journey. A line that matches nothing is skipped.
pub fn parse(source: &str) -> Journey {
    let mut journey = Journey::default();
    for raw in source.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() || opens_with(line, "journey") {
            continue;
        }
        if let Some(title) = after_keyword(line, "title") {
            journey.title = Some(title.to_string());
            continue;
        }
        if let Some(name) = after_keyword(line, "section") {
            journey.sections.push(Section {
                name: name.to_string(),
                tasks: Vec::new(),
            });
            continue;
        }
        if let Some(task) = parse_task(line) {
            // A task before any section opens an implicit unnamed one, so
            // nothing written is lost.
            if journey.sections.is_empty() {
                journey.sections.push(Section {
                    name: String::new(),
                    tasks: Vec::new(),
                });
            }
            if let Some(section) = journey.sections.last_mut() {
                section.tasks.push(task);
            }
        }
    }
    journey
}

#[cfg(test)]
mod tests {
    use super::*;

    const JOURNEY: &str = "journey\n\
        title My working day\n\
        section Go to work\n\
          Make tea: 5: Me\n\
          Go upstairs: 3: Me, Cat\n\
        section Be at work\n\
          Do work: 1: Me, Cat";

    #[test]
    fn a_whole_journey_reads() {
        let journey = parse(JOURNEY);
        assert_eq!(journey.title.as_deref(), Some("My working day"));
        assert_eq!(journey.sections.len(), 2);
        assert_eq!(journey.sections[0].tasks.len(), 2);
        assert_eq!(journey.sections[0].tasks[1].actors, ["Me", "Cat"]);
        assert_eq!(journey.sections[1].tasks[0].score, 1);
    }

    #[test]
    fn a_task_before_any_section_lands_in_an_implicit_one() {
        let journey = parse("journey\nOrphan: 3: Me");
        assert_eq!(journey.sections.len(), 1);
        assert_eq!(journey.sections[0].name, "");
        assert_eq!(journey.sections[0].tasks.len(), 1);
    }

    #[test]
    fn the_actor_list_is_optional() {
        assert!(parse("journey\nAlone: 4").sections[0].tasks[0]
            .actors
            .is_empty());
        assert!(parse("journey\nAlone: 4:").sections[0].tasks[0]
            .actors
            .is_empty());
    }

    #[test]
    fn a_score_off_the_scale_is_pulled_onto_it() {
        let scores: Vec<i32> = parse("journey\nA: 9\nB: 0\nC: -3").sections[0]
            .tasks
            .iter()
            .map(|t| t.score)
            .collect();
        assert_eq!(scores, [5, 1, 1]);
    }

    #[test]
    fn an_actor_name_may_contain_a_colon() {
        let task = &parse("journey\nA: 3: Me: the cat").sections[0].tasks[0];
        assert_eq!(task.actors, ["Me: the cat"]);
    }

    #[test]
    fn a_row_without_a_readable_score_is_not_a_task() {
        for row in ["No colon at all", ": 3: Me", "A: not a number", "A: 3.5"] {
            let journey = parse(&format!("journey\nsection S\n{row}"));
            assert!(journey.sections[0].tasks.is_empty(), "{row}");
        }
    }

    #[test]
    fn an_empty_section_stays_in_the_list() {
        // It draws no band, but it still consumes a palette slot — dropping it
        // here would shift every later section's colour.
        let journey = parse("journey\nsection Empty\nsection Full\nA: 3");
        assert_eq!(journey.sections.len(), 2);
        assert!(journey.sections[0].tasks.is_empty());
    }

    #[test]
    fn a_title_and_a_section_name_keep_their_quotes() {
        // The reference does not strip them here, and a name is shown verbatim.
        assert_eq!(
            parse("journey\ntitle \"Quoted\"").title.as_deref(),
            Some("\"Quoted\"")
        );
    }

    #[test]
    fn a_comment_is_stripped_before_the_line_is_read() {
        assert_eq!(
            parse("journey\nA: 3: Me %% a note").sections[0].tasks[0].actors,
            ["Me"]
        );
    }

    #[test]
    fn nothing_in_yields_an_empty_journey() {
        assert_eq!(parse(""), Journey::default());
        assert_eq!(parse("journey"), Journey::default());
    }
}
