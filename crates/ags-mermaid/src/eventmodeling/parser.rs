//! Reading `eventmodeling` source.
//!
//! ```text
//! eventmodeling
//!   title <text>
//!   tf 01 evt OrderPlaced           the compact spelling
//!   timeframe 02 command PlaceOrder the relaxed one
//!   tf 03 rmo Orders { id, total }  an inline data block, stripped
//! ```
//!
//! Each kind has two spellings and they mean the same thing, so both map to one
//! entity before anything downstream sees them.

use super::types::{Entity, Frame, Model};
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

/// Both spellings of each kind, compact and relaxed.
fn entity_of(token: &str) -> Option<Entity> {
    match token.to_ascii_lowercase().as_str() {
        "ui" => Some(Entity::Ui),
        "pcr" | "processor" => Some(Entity::Processor),
        "cmd" | "command" => Some(Entity::Command),
        "rmo" | "readmodel" | "rm" => Some(Entity::ReadModel),
        "evt" | "event" => Some(Entity::Event),
        _ => None,
    }
}

/// Drop every `{ … }` data block from a name.
fn strip_data(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// An id no other frame has claimed.
fn unique_id(base: &str, used: &mut Vec<String>) -> String {
    let mut id = base.to_string();
    let mut n = 2usize;
    while used.contains(&id) {
        id = format!("{base}#{n}");
        n += 1;
    }
    used.push(id.clone());
    id
}

/// A `tf`/`timeframe <number> <type> <name>` line.
fn parse_frame(line: &str, used: &mut Vec<String>) -> Option<Frame> {
    let rest = after_keyword(line, "timeframe").or_else(|| after_keyword(line, "tf"))?;
    let mut parts = rest.splitn(3, char::is_whitespace);
    let number = parts.next()?.trim();
    let kind = parts.next()?.trim();
    let name = strip_data(parts.next()?);
    // The type has to be letters only, so a two-token line is not read as one
    // with an empty name.
    if number.is_empty() || name.is_empty() || !kind.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let entity = entity_of(kind)?;
    // Ordering follows the digits in the number; a number with none falls back
    // to the position it was written at, so it still lands somewhere stable.
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();
    let numeric = digits.parse().unwrap_or(used.len());
    Some(Frame {
        number: unique_id(number, used),
        numeric,
        entity,
        name,
    })
}

/// Parse an event model. A line that matches nothing is skipped.
pub fn parse(source: &str) -> Model {
    let mut model = Model::default();
    let mut used: Vec<String> = Vec::new();
    for raw in source.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() || opens_with(line, "eventmodeling") {
            continue;
        }
        if let Some(title) = after_keyword(line, "title") {
            model.title = Some(title.to_string());
            continue;
        }
        if let Some(frame) = parse_frame(line, &mut used) {
            model.frames.push(frame);
        }
    }
    model
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = "eventmodeling\n\
        title Ordering\n\
        tf 01 ui Basket\n\
        tf 02 cmd PlaceOrder\n\
        tf 03 evt OrderPlaced { id, total }\n\
        timeframe 04 readmodel Orders";

    #[test]
    fn a_whole_model_reads() {
        let model = parse(MODEL);
        assert_eq!(model.title.as_deref(), Some("Ordering"));
        assert_eq!(model.frames.len(), 4);
        assert_eq!(model.frames[0].entity, Entity::Ui);
        assert_eq!(model.frames[3].entity, Entity::ReadModel);
    }

    #[test]
    fn both_spellings_of_a_kind_mean_the_same_thing() {
        let compact = parse("eventmodeling\ntf 1 cmd A");
        let relaxed = parse("eventmodeling\ntf 1 command A");
        assert_eq!(compact.frames[0].entity, relaxed.frames[0].entity);
        // And `tf` and `timeframe` are the same keyword.
        assert_eq!(parse("eventmodeling\ntimeframe 1 cmd A").frames.len(), 1);
    }

    #[test]
    fn an_inline_data_block_is_stripped_from_the_name() {
        assert_eq!(parse(MODEL).frames[2].name, "OrderPlaced");
        // Including one in the middle of a name.
        assert_eq!(
            parse("eventmodeling\ntf 1 evt A { x } B").frames[0].name,
            "A  B"
        );
    }

    #[test]
    fn the_digits_in_a_number_decide_the_order() {
        let model = parse("eventmodeling\ntf 10 ui A\ntf 02 ui B");
        assert_eq!(model.frames[0].numeric, 10);
        assert_eq!(model.frames[1].numeric, 2);
    }

    #[test]
    fn a_number_with_no_digits_still_lands_somewhere_stable() {
        let model = parse("eventmodeling\ntf a ui A\ntf b ui B");
        assert_eq!(model.frames[0].numeric, 0);
        assert_eq!(model.frames[1].numeric, 1);
    }

    #[test]
    fn a_repeated_number_is_still_told_apart() {
        let model = parse("eventmodeling\ntf 1 ui A\ntf 1 ui B");
        assert_eq!(model.frames[0].number, "1");
        assert_eq!(model.frames[1].number, "1#2");
    }

    #[test]
    fn each_kind_lands_in_its_own_lane() {
        let model =
            parse("eventmodeling\ntf 1 ui A\ntf 2 pcr B\ntf 3 cmd C\ntf 4 rmo D\ntf 5 evt E");
        let lanes: Vec<&str> = model
            .frames
            .iter()
            .map(|f| f.entity.lane().label())
            .collect();
        assert_eq!(
            lanes,
            [
                "UI / Automation",
                "UI / Automation",
                "Command / Read Model",
                "Command / Read Model",
                "Events",
            ]
        );
    }

    #[test]
    fn a_line_that_is_not_a_frame_is_skipped() {
        for line in ["tf 1 nonsense A", "tf 1 ui", "tf 1", "not a frame"] {
            assert!(
                parse(&format!("eventmodeling\n{line}")).frames.is_empty(),
                "{line}"
            );
        }
    }

    #[test]
    fn nothing_in_yields_an_empty_model() {
        assert_eq!(parse(""), Model::default());
        assert_eq!(parse("eventmodeling"), Model::default());
    }
}
