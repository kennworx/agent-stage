//! Feedback arriving from an HTML form.
//!
//! The served page posts with `<form method="post">` rather than through a
//! script, so what reaches the host is `application/x-www-form-urlencoded` and not
//! the JSON the viewer used to send. Both forms funnel through
//! [`FeedbackItem::new`], so the no-target rule is enforced once wherever an item
//! comes from.
//!
//! Hand-decoded rather than pulled from a crate: the grammar is two rules — `+` is
//! a space, `%XX` is a byte — and the host already declines a dependency it can
//! write in thirty lines.

use crate::model::{FeedbackItem, FeedbackKind, SubTarget};

/// One `key=value` pair's value, decoded.
///
/// A stray `%` or a truncated escape is kept literally rather than dropped: this
/// is reviewer prose, and losing a character silently is worse than showing the
/// `%` they typed.
fn decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes.get(i) {
            Some(b'+') => {
                out.push(b' ');
                i += 1;
            }
            Some(b'%') => {
                let hex = raw
                    .get(i + 1..i + 3)
                    .and_then(|h| u8::from_str_radix(h, 16).ok());
                if let Some(byte) = hex {
                    out.push(byte);
                    i += 3;
                } else {
                    out.push(b'%');
                    i += 1;
                }
            }
            Some(&byte) => {
                out.push(byte);
                i += 1;
            }
            None => break,
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The value of `key` in a urlencoded body, decoded.
fn field(body: &str, key: &str) -> Option<String> {
    body.split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| decode(k) == key)
        .map(|(_, v)| decode(v))
}

/// Parse one submitted item from a form post.
///
/// `kind` decides what the body means: an `annotation` carries the reviewer's
/// prose, an `answer` carries the option they chose. `sub` names a diagram node
/// when the reviewer aimed the comment at one, and is absent for a block-level
/// note — an empty string reads as absent, because that is what an untouched
/// hidden input submits.
///
/// # Errors
/// When the form names no block, carries no body, or names a kind that is not a
/// reviewer's to send.
pub fn parse_feedback_form(body: &str) -> Result<FeedbackItem, String> {
    let block_id = field(body, "block_id").unwrap_or_default();
    let text = field(body, "body").unwrap_or_default();
    if text.trim().is_empty() {
        return Err("feedback carries no body".to_string());
    }
    let kind = match field(body, "kind").as_deref() {
        Some("answer") => FeedbackKind::Answer,
        Some("annotation") | None => FeedbackKind::Annotation,
        Some(other) => return Err(format!("'{other}' is not a kind a reviewer sends")),
    };
    let sub = field(body, "sub")
        .filter(|s| !s.trim().is_empty())
        .map(SubTarget::Node);
    FeedbackItem::new(block_id, sub, kind, text)
        .map_err(|_no_target| "feedback names no resolvable target".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plus_is_a_space_and_a_percent_pair_is_a_byte() {
        assert_eq!(decode("a+b"), "a b");
        assert_eq!(decode("a%20b"), "a b");
        assert_eq!(decode("%C3%A9"), "é");
        assert_eq!(decode("plain"), "plain");
        assert_eq!(decode(""), "");
    }

    #[test]
    fn a_broken_escape_keeps_the_character_the_reviewer_typed() {
        // Dropping it would silently edit their prose.
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("50%z9"), "50%z9");
        assert_eq!(decode("%2"), "%2");
    }

    #[test]
    fn a_field_is_found_by_its_decoded_name() {
        let body = "block_id=flow&body=hello+there&kind=annotation";
        assert_eq!(field(body, "block_id").as_deref(), Some("flow"));
        assert_eq!(field(body, "body").as_deref(), Some("hello there"));
        assert_eq!(field(body, "missing"), None);
    }

    #[test]
    fn a_valueless_pair_is_not_a_field() {
        assert_eq!(field("novalue&a=1", "novalue"), None);
        assert_eq!(field("novalue&a=1", "a").as_deref(), Some("1"));
    }

    #[test]
    fn an_annotation_is_the_default_kind() {
        let item = parse_feedback_form("block_id=flow&body=looks+wrong").expect("parses");
        assert_eq!(item.block_id, "flow");
        assert_eq!(item.kind, FeedbackKind::Annotation);
        assert_eq!(item.body, "looks wrong");
        assert!(item.sub_target.is_none());
    }

    #[test]
    fn a_node_target_rides_along_when_the_reviewer_aimed_at_one() {
        let item = parse_feedback_form("block_id=flow&sub=Auth&body=why+here").expect("parses");
        assert_eq!(item.sub_target, Some(SubTarget::Node("Auth".into())));
    }

    #[test]
    fn an_empty_sub_reads_as_no_sub() {
        // What an untouched hidden input submits.
        let item = parse_feedback_form("block_id=flow&sub=&body=x").expect("parses");
        assert!(item.sub_target.is_none());
    }

    #[test]
    fn an_answer_is_recorded_as_one() {
        let item = parse_feedback_form("block_id=q1&kind=answer&body=SQLite").expect("parses");
        assert_eq!(item.kind, FeedbackKind::Answer);
        assert_eq!(item.body, "SQLite");
    }

    #[test]
    fn a_post_with_no_block_is_refused() {
        parse_feedback_form("body=orphan").unwrap_err();
        parse_feedback_form("block_id=+&body=orphan").unwrap_err();
    }

    #[test]
    fn an_empty_body_is_refused_rather_than_recorded_blank() {
        parse_feedback_form("block_id=flow&body=").unwrap_err();
        parse_feedback_form("block_id=flow&body=+++").unwrap_err();
        parse_feedback_form("block_id=flow").unwrap_err();
    }

    #[test]
    fn a_kind_a_reviewer_does_not_send_is_refused() {
        // `finding` is the render gate's, not a person's.
        let err = parse_feedback_form("block_id=f&kind=finding&body=x").unwrap_err();
        assert!(err.contains("finding"), "{err}");
    }
}
