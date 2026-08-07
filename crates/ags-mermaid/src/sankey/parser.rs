//! Reading `sankey` source.
//!
//! ```text
//! sankey | sankey-beta
//! source,target,value           the body is CSV, three columns
//! "Field, quoted",Other,124.729 a quoted field may hold a comma
//! ```
//!
//! A row whose third column is not a finite number is skipped, which is what
//! silently absorbs the optional `source,target,value` header row.

use super::types::{Diagram, Link};
use crate::keyword::opens_with;

/// Everything before a `%%` comment.
fn strip_comment(line: &str) -> &str {
    line.split("%%").next().unwrap_or(line)
}

/// Split one CSV row into fields.
///
/// A quoted field may hold commas; a doubled quote inside one is a literal
/// quote. Quotes are structural, so they are dropped wherever they appear —
/// including mid-field, which is what the reference's single pass does.
fn csv_row(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut i = 0usize;
    while i < chars.len() {
        let Some(&c) = chars.get(i) else { break };
        if quoted {
            if c == '"' {
                if chars.get(i + 1) == Some(&'"') {
                    current.push('"');
                    i += 1;
                } else {
                    quoted = false;
                }
            } else {
                current.push(c);
            }
        } else if c == '"' {
            quoted = true;
        } else if c == ',' {
            fields.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
        i += 1;
    }
    fields.push(current);
    fields
}

/// Parse a sankey diagram. A row that is not three usable columns is skipped.
pub fn parse(source: &str) -> Diagram {
    let mut diagram = Diagram::default();
    for raw in source.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() || opens_with(line, "sankey-beta") || opens_with(line, "sankey") {
            continue;
        }
        let fields = csv_row(line);
        let (Some(source), Some(target), Some(value)) =
            (fields.first(), fields.get(1), fields.get(2))
        else {
            continue;
        };
        let (source, target) = (source.trim(), target.trim());
        // Read strictly rather than taking a leading number off the front of
        // the field: a value column that is not a number is a header row or a
        // typo, and guessing at it would draw a band nobody asked for.
        let Ok(value) = value.trim().parse::<f64>() else {
            continue;
        };
        if source.is_empty() || target.is_empty() || !value.is_finite() {
            continue;
        }
        for name in [source, target] {
            if !diagram.nodes.iter().any(|n| n == name) {
                diagram.nodes.push(name.to_string());
            }
        }
        diagram.links.push(Link {
            source: source.to_string(),
            target: target.to_string(),
            value,
        });
    }
    diagram
}

#[cfg(test)]
mod tests {
    use super::*;

    fn links(source: &str) -> Vec<(String, String, f64)> {
        parse(source)
            .links
            .into_iter()
            .map(|l| (l.source, l.target, l.value))
            .collect()
    }

    #[test]
    fn a_row_reads_as_source_target_and_value() {
        assert_eq!(
            links("sankey-beta\nAgricultural waste,Bio-conversion,124.729"),
            [(
                "Agricultural waste".to_string(),
                "Bio-conversion".to_string(),
                124.729
            )]
        );
    }

    #[test]
    fn nodes_are_collected_once_each_in_order_of_first_appearance() {
        let diagram = parse("sankey\nA,B,1\nB,C,2\nA,C,3");
        assert_eq!(diagram.nodes, ["A", "B", "C"]);
    }

    #[test]
    fn a_quoted_field_may_hold_a_comma() {
        let out = links("sankey\n\"Waste, agricultural\",Bio,1");
        assert_eq!(out[0].0, "Waste, agricultural");
    }

    #[test]
    fn a_doubled_quote_inside_a_quoted_field_is_a_literal_one() {
        assert_eq!(
            links("sankey\n\"He said \"\"hi\"\"\",B,1")[0].0,
            "He said \"hi\""
        );
    }

    #[test]
    fn the_optional_header_row_is_absorbed_rather_than_drawn() {
        let diagram = parse("sankey-beta\nsource,target,value\nA,B,1");
        assert_eq!(diagram.links.len(), 1);
        assert_eq!(diagram.nodes, ["A", "B"]);
    }

    #[test]
    fn a_row_that_cannot_be_a_flow_is_skipped() {
        for row in [
            "A,B",
            "A,,1",
            ",B,1",
            "A,B,",
            "A,B,NaN",
            "A,B,inf",
            "A,B,12abc",
        ] {
            assert!(
                parse(&format!("sankey\n{row}")).links.is_empty(),
                "{row} was read as a flow"
            );
        }
    }

    #[test]
    fn a_negative_or_zero_value_is_still_a_flow() {
        // Refusing them here would silently drop a row the author wrote; the
        // layout is what decides how thin a band may get.
        assert!(links("sankey\nA,B,0")[0].2.abs() < 1e-9);
        assert!((links("sankey\nA,B,-5")[0].2 + 5.0).abs() < 1e-9);
    }

    #[test]
    fn a_fourth_column_is_ignored() {
        assert_eq!(links("sankey\nA,B,1,extra").len(), 1);
    }

    #[test]
    fn both_header_spellings_are_skipped_but_a_node_named_sankeys_is_not() {
        assert!(parse("sankey").links.is_empty());
        assert!(parse("sankey-beta").links.is_empty());
        assert_eq!(parse("sankey\nsankeys,B,1").nodes[0], "sankeys");
    }

    #[test]
    fn a_comment_is_stripped_before_the_row_is_read() {
        assert_eq!(links("sankey\nA,B,1 %% a note").len(), 1);
    }

    #[test]
    fn nothing_in_yields_an_empty_graph() {
        assert_eq!(parse(""), Diagram::default());
        assert_eq!(parse("sankey"), Diagram::default());
    }
}
