//! What each block type's body has to look like.
//!
//! One function per type rather than a table, because the rules have nothing in
//! common: a question is a prompt and its options, a table is rows that agree on
//! their width, a theme is a set of tokens, and a diagram is checked by drawing
//! it.

use ags_mermaid::DiagramType;

use crate::block::{Block, ValidationError, ValidationKind};

use super::attr_value;

pub(super) fn validate_body(block: &Block, anchor: &str) -> Vec<ValidationError> {
    match block.type_token.as_str() {
        "mermaid" => validate_mermaid_body(block, anchor),
        "code" => non_empty_body(block, anchor, "code"),
        "question" => validate_question_body(block, anchor),
        "table" => validate_table_body(block, anchor),
        "html" => crate::html::check_html(&block.body, anchor),
        "note" => non_empty_body(block, anchor, "note"),
        "theme" => validate_theme_body(block, anchor),
        _ => Vec::new(),
    }
}

/// The semantic tokens an agent may set from a `theme` block — shadcn/ui's names,
/// so themes read in the same vocabulary the wider ecosystem uses.
pub(crate) const THEME_TOKENS: &[&str] = &[
    "background",
    "foreground",
    "card",
    "muted-foreground",
    "border",
    "primary",
    "primary-foreground",
];

/// A `theme` block re-themes the page via tokens only: each non-blank line is
/// `token: #hex` (token from [`THEME_TOKENS`]), a single `seed: #hex` accent, or a
/// `dark:`/`light:` section header grouping the lines that follow — so one theme
/// carries both a dark and a light set (as deltas over the base mode; lines before
/// any header apply to both). A `seed` is the accent the renderer expands into a
/// full per-mode palette (OKLCH lightness ramp); how it derives the other tokens is
/// a rendering concern, so Gate 1 only checks the seed is a hex color. This keeps
/// agent theming safe by construction (no raw colors leak into content) — unknown
/// tokens and non-hex values are rejected.
pub(super) fn validate_theme_body(block: &Block, anchor: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut tokens = false;
    for raw in block.body.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        // A `dark:`/`light:` line is a mode section header, not a token.
        if matches!(line.trim_end_matches(':').trim(), "dark" | "light") {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            errors.push(ValidationError::new(
                anchor,
                ValidationKind::ThemeToken,
                format!("theme line '{line}' must be 'token: #hex' or a 'dark:'/'light:' section"),
            ));
            continue;
        };
        tokens = true;
        let (key, value) = (key.trim(), value.trim());
        // `seed` is the accent the renderer expands into a palette — not one of the
        // applied tokens, but still a color, so it is hex-checked like a token.
        if key == "seed" {
            if !is_hex_color(value) {
                errors.push(ValidationError::new(
                    anchor,
                    ValidationKind::ThemeToken,
                    format!("theme 'seed' must be a hex color, got '{value}'"),
                ));
            }
        } else if !THEME_TOKENS.contains(&key) {
            errors.push(ValidationError::new(
                anchor,
                ValidationKind::ThemeToken,
                format!(
                    "'{key}' is not a theme token or 'seed' ({})",
                    THEME_TOKENS.join(", ")
                ),
            ));
        } else if !is_hex_color(value) {
            errors.push(ValidationError::new(
                anchor,
                ValidationKind::ThemeToken,
                format!("theme token '{key}' must be a hex color, got '{value}'"),
            ));
        }
    }
    if !tokens {
        errors.push(ValidationError::new(
            anchor,
            ValidationKind::EmptyBody,
            "a 'theme' block needs at least one 'token: #hex' or 'seed: #hex' line",
        ));
    }
    errors
}

/// Whether `s` is a `#rgb` / `#rgba` / `#rrggbb` / `#rrggbbaa` hex color — the four
/// CSS hex lengths, matching the viewer's `parseTheme` grammar so the two gates agree.
/// Shared with [`crate::style`] so themed-content color detection uses one grammar.
pub(crate) fn is_hex_color(s: &str) -> bool {
    let hex = s.strip_prefix('#').unwrap_or_default();
    matches!(hex.len(), 3 | 4 | 6 | 8) && hex.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Require a non-empty body (mermaid parse-validity is deferred to the browser gate).
/// A `mermaid` block must name a diagram the renderer draws, and its source must
/// parse into one.
///
/// Gate 1 asks the renderer rather than keeping a list of its own: the supported
/// set is [`DiagramType::ALL`], and the only authority on whether a source parses
/// is the parser. This is what the catalog promises and what the page will do a
/// moment later, so an artifact that passes here has a drawing on every diagram.
///
/// Deliberately *not* checked here: the legibility violations `render_svg` returns
/// alongside the SVG. Those describe a drawing that exists but reads badly, which
/// is a finding for the agent, not a reason to refuse a review — and they are noisy
/// enough today that gating on them would refuse artifacts a human would accept.
pub(super) fn validate_mermaid_body(block: &Block, anchor: &str) -> Vec<ValidationError> {
    let empty = non_empty_body(block, anchor, "mermaid");
    if !empty.is_empty() {
        return empty;
    }
    match ags_mermaid::render_svg(&block.body, &ags_mermaid::Options::default()) {
        Ok(_) => Vec::new(),
        Err(err) => vec![diagram_error(anchor, &err)],
    }
}

/// Turn a render failure into the Gate-1 error that reports it.
///
/// Split from the call above so both arms are reachable from a test. Only
/// `UnknownType` can be provoked through a source today — every parser in the
/// renderer is lenient, so nothing constructs a `Malformed` — and a test that
/// could only reach one arm through `render_svg` would leave the other untested
/// while it waits for a parser strict enough to trip it.
pub(super) fn diagram_error(anchor: &str, err: &ags_mermaid::RenderError) -> ValidationError {
    match err {
        ags_mermaid::RenderError::UnknownType { found, suggestion } => ValidationError::new(
            anchor,
            ValidationKind::DiagramType,
            unknown_diagram_detail(found, *suggestion),
        ),
        ags_mermaid::RenderError::Malformed { line, message } => ValidationError::new(
            anchor,
            ValidationKind::DiagramMalformed,
            format!("diagram source does not parse at line {line}: {message}"),
        ),
    }
}

/// Explain an unrecognised diagram header, naming the supported set.
///
/// The set is spelled out rather than pointed at, because the reader is an agent
/// deciding what to write next and a message that says "see the catalog" costs it
/// another round trip.
pub(super) fn unknown_diagram_detail(found: &str, suggestion: Option<&str>) -> String {
    if let Some(hint) = suggestion {
        return format!("unknown diagram type '{found}' — did you mean '{hint}'?");
    }
    let mut names: Vec<&str> = DiagramType::ALL.iter().map(|k| k.keyword()).collect();
    names.sort_unstable_by_key(|n| n.to_ascii_lowercase());
    if found.is_empty() {
        return format!(
            "diagram source declares no type on its first line; supported: {}",
            names.join(", ")
        );
    }
    format!(
        "unknown diagram type '{found}'; supported: {}",
        names.join(", ")
    )
}

pub(super) fn non_empty_body(block: &Block, anchor: &str, kind: &str) -> Vec<ValidationError> {
    if block.body.trim().is_empty() {
        vec![ValidationError::new(
            anchor,
            ValidationKind::EmptyBody,
            format!("a '{kind}' block needs a non-empty body"),
        )]
    } else {
        Vec::new()
    }
}

/// A `question` needs a prompt and, for choice types, at least two options.
pub(super) fn validate_question_body(block: &Block, anchor: &str) -> Vec<ValidationError> {
    let (prompt_lines, options) = split_prompt_options(&block.body);
    let mut errors = Vec::new();
    if prompt_lines == 0 {
        errors.push(ValidationError::new(
            anchor,
            ValidationKind::EmptyBody,
            "a 'question' block needs a prompt line before its options",
        ));
    }
    if let Some(qtype) = attr_value(block, "type") {
        if matches!(qtype, "radio" | "checkbox" | "select") && options < 2 {
            errors.push(ValidationError::new(
                anchor,
                ValidationKind::QuestionOptions,
                format!("a '{qtype}' question needs at least two options, found {options}"),
            ));
        }
    }
    errors
}

/// Count prompt lines (non-empty, non-list) and option lines (markdown `-`/`*` list).
pub(super) fn split_prompt_options(body: &str) -> (usize, usize) {
    let mut prompt = 0;
    let mut options = 0;
    for line in body.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t.starts_with("- ") || t.starts_with("* ") {
            options += 1;
        } else {
            prompt += 1;
        }
    }
    (prompt, options)
}

/// A `table` body must be rectangular: every data row's cell count equals the header's.
pub(super) fn validate_table_body(block: &Block, anchor: &str) -> Vec<ValidationError> {
    let rows: Vec<Vec<String>> = block
        .body
        .lines()
        .filter(|l| l.contains('|'))
        .map(parse_row)
        .collect();
    let Some(header) = rows.first() else {
        return vec![ValidationError::new(
            anchor,
            ValidationKind::EmptyBody,
            "a 'table' block needs at least a header row",
        )];
    };
    let header_arity = header.len();
    let mut errors = Vec::new();
    for (idx, row) in rows.iter().enumerate().skip(1) {
        if is_separator_row(row) {
            continue;
        }
        if row.len() != header_arity {
            errors.push(ValidationError::new(
                anchor,
                ValidationKind::TableArity,
                format!(
                    "row {} has {} cells but the header has {header_arity}",
                    idx + 1,
                    row.len()
                ),
            ));
        }
    }
    errors
}

/// Split a markdown table row into trimmed cells, ignoring the outer pipes.
pub(super) fn parse_row(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').map(|c| c.trim().to_string()).collect()
}

/// Whether every cell is a markdown alignment marker (`---`, `:--`, `:-:`, `--:`).
pub(super) fn is_separator_row(cells: &[String]) -> bool {
    !cells.is_empty() && cells.iter().all(|c| is_dash_cell(c))
}

pub(super) fn is_dash_cell(cell: &str) -> bool {
    let core = cell.trim().trim_matches(':');
    !core.is_empty() && core.chars().all(|c| c == '-')
}
