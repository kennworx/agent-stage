//! Domain model for agent-authored artifact blocks and Gate-1 validation errors.
//!
//! A parsed artifact is a list of [`Block`]s (fenced blocks) plus any structural
//! errors found while parsing. Validation turns rule violations into
//! [`ValidationError`]s, which the CLI serializes to TOON.

use serde::Serialize;

/// The closed set of **addressable** block types — the ones that carry an id, a
/// schema, and a review affordance.
///
/// A fence whose first info-string token is not one of these is not a block at
/// all: it stays in the surrounding implicit prose, and the page renders it as
/// an ordinary fenced code block.
///
/// This is the single source of truth shared by [`crate::parse`] (which
/// classifies), [`crate::validate`] (which enforces), [`crate::catalog`] (which
/// documents) and [`crate::page`] (which draws), so they cannot drift. It used to
/// be mirrored by a TypeScript copy in the viewer, which is exactly the drift this
/// list exists to prevent; the viewer is gone and the copy with it.
pub const BLOCK_TYPES: &[&str] = &[
    "mermaid", "question", "table", "code", "html", "note", "theme",
];

/// Whether a fence info-string's first token names an addressable block.
#[must_use]
pub fn is_block_type(type_token: &str) -> bool {
    BLOCK_TYPES.contains(&type_token)
}

/// A parsed fenced block, before validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Raw first info-string token — the claimed block type, e.g. `"mermaid"`.
    pub type_token: String,
    /// The `#id` value (without the leading `#`), when present.
    pub id: Option<String>,
    /// Attributes following the type and optional id.
    pub attrs: Vec<Attr>,
    /// Block body (text between the fences), with the trailing newline trimmed.
    pub body: String,
    /// 1-based line of the opening fence, for human-facing anchoring.
    pub line: usize,
    /// 1-based line just past the block — the line a following segment starts on.
    ///
    /// Recorded by the parser rather than re-derived from [`Self::body`], because
    /// the body cannot answer the question. It is joined with `\n`, so a trailing
    /// blank line vanishes into a trailing newline that `lines()` does not count,
    /// and a body of one blank line is indistinguishable from no body at all.
    /// Both cases made a re-derived span land on the closing fence, which then
    /// leaked into the next prose run and opened a code block that swallowed the
    /// rest of the document.
    pub end: usize,
    /// 0-based ordinal among fenced blocks, used to anchor id-less blocks.
    pub ordinal: usize,
}

impl Block {
    /// Stable anchor used in error rows: the `#id` when present, else
    /// `<type>@<ordinal>` so an id-less offending block is still addressable.
    #[must_use]
    pub fn anchor(&self) -> String {
        match &self.id {
            Some(id) => format!("#{id}"),
            None => format!("{}@{}", self.type_token, self.ordinal),
        }
    }
}

/// A single info-string attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attr {
    /// Attribute key (the part before `=`, or the whole token for a flag).
    pub key: String,
    /// Attribute value.
    pub value: AttrValue,
}

/// The value carried by an [`Attr`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrValue {
    /// A bare flag, e.g. `collapsible` or `required`.
    Flag,
    /// A `key=value` pair (value already unquoted).
    Value(String),
}

/// One rule violation, anchored to the offending block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationError {
    /// Offending block anchor (see [`Block::anchor`]); serialized as `id`.
    #[serde(rename = "id")]
    pub anchor: String,
    /// Machine-readable failure category.
    pub kind: ValidationKind,
    /// Human-actionable detail.
    pub detail: String,
}

impl ValidationError {
    /// Construct an error from a block anchor, kind, and detail.
    pub fn new(anchor: impl Into<String>, kind: ValidationKind, detail: impl Into<String>) -> Self {
        Self {
            anchor: anchor.into(),
            kind,
            detail: detail.into(),
        }
    }
}

/// The category of a [`ValidationError`]. Serialized as a stable kebab-case token
/// in the TOON `kind` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ValidationKind {
    /// An opening fence never closed before end of input.
    UnclosedFence,
    /// A malformed info-string token (not `#id`, `key=value`, or a bare flag).
    InfoGrammar,
    /// Block type is not in the closed v1 set.
    UnknownType,
    /// A fence type is not a block type but is one edit away from one — almost
    /// certainly a typo that would otherwise degrade silently into prose.
    NearMissType,
    /// Two blocks share the same `#id`.
    DuplicateId,
    /// An attribute key is not valid for this block type.
    UnknownAttr,
    /// An attribute value is outside its allowed set.
    BadAttrValue,
    /// `feedback` is set but the block carries no `#id`.
    FeedbackNeedsId,
    /// A required attribute is missing.
    MissingAttr,
    /// A block body that must be non-empty is empty.
    EmptyBody,
    /// A choice question has fewer than two options.
    QuestionOptions,
    /// A table row's cell count differs from the header's.
    TableArity,
    /// A `code` block has no known `lang`.
    CodeLang,
    /// A `mermaid` block's first line names no diagram this renderer draws.
    DiagramType,
    /// A `mermaid` block names a diagram this renderer draws, but its source does
    /// not parse — so there would be no drawing on the page.
    DiagramMalformed,
    /// An HTML chunk attribute uses an unsafe URL scheme (e.g. `javascript:`).
    HtmlUnsafeUrl,
    /// An HTML chunk uses a non-whitelisted tag.
    HtmlDisallowedTag,
    /// An HTML chunk contains a `<script>` element.
    HtmlScript,
    /// An HTML chunk carries an `on*` event-handler attribute.
    HtmlEventHandler,
    /// Themed content (an `html` block) uses a hardcoded color literal instead of a token.
    HtmlHardcodedColor,
    /// Themed content sets a `font-family` — the renderer owns the font.
    HtmlFontFamily,
    /// Themed content uses absolute/fixed positioning, escaping the renderer's flow layout.
    HtmlPositioning,
    /// A custom-diagram node (`.ui-diagram-node`) carries no `data-id`, so a human
    /// annotation could not be keyed to it.
    HtmlDiagramNodeNeedsId,
    /// A `theme` block line is not `<token>: #hex` from the allowed token set.
    ThemeToken,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_prefers_id() {
        let b = Block {
            type_token: "mermaid".into(),
            id: Some("flow".into()),
            attrs: vec![],
            body: String::new(),
            line: 3,
            end: 5,
            ordinal: 0,
        };
        assert_eq!(b.anchor(), "#flow");
    }

    #[test]
    fn anchor_falls_back_to_type_and_ordinal() {
        let b = Block {
            type_token: "table".into(),
            id: None,
            attrs: vec![],
            body: String::new(),
            line: 9,
            end: 11,
            ordinal: 2,
        };
        assert_eq!(b.anchor(), "table@2");
    }

    #[test]
    fn error_new_converts_into_strings() {
        let e = ValidationError::new("#a", ValidationKind::EmptyBody, "body is empty");
        assert_eq!(e.anchor, "#a");
        assert_eq!(e.kind, ValidationKind::EmptyBody);
        assert_eq!(e.detail, "body is empty");
    }

    #[test]
    fn attr_value_variants_are_distinct() {
        assert_ne!(AttrValue::Flag, AttrValue::Value("x".into()));
    }

    #[test]
    fn kinds_serialize_to_unique_kebab_tokens() {
        // Enumerate all kinds so the derived Serialize is exercised and tokens
        // are unique.
        let all = [
            ValidationKind::UnclosedFence,
            ValidationKind::InfoGrammar,
            ValidationKind::UnknownType,
            ValidationKind::NearMissType,
            ValidationKind::DuplicateId,
            ValidationKind::UnknownAttr,
            ValidationKind::BadAttrValue,
            ValidationKind::FeedbackNeedsId,
            ValidationKind::MissingAttr,
            ValidationKind::EmptyBody,
            ValidationKind::QuestionOptions,
            ValidationKind::TableArity,
            ValidationKind::CodeLang,
            ValidationKind::DiagramType,
            ValidationKind::DiagramMalformed,
            ValidationKind::HtmlUnsafeUrl,
            ValidationKind::HtmlDisallowedTag,
            ValidationKind::HtmlScript,
            ValidationKind::HtmlEventHandler,
            ValidationKind::HtmlHardcodedColor,
            ValidationKind::HtmlFontFamily,
            ValidationKind::HtmlPositioning,
            ValidationKind::HtmlDiagramNodeNeedsId,
            ValidationKind::ThemeToken,
        ];
        let mut seen = std::collections::HashSet::new();
        for k in all {
            let token = serde_json::to_string(&k).unwrap();
            assert!(seen.insert(token.clone()), "duplicate token {token}");
        }
        assert_eq!(seen.len(), all.len());
        assert_eq!(
            serde_json::to_string(&ValidationKind::HtmlUnsafeUrl).unwrap(),
            "\"html-unsafe-url\""
        );
    }
}
