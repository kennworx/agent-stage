//! Answering one request.
//!
//! Nothing is served from disk: the page is built per request from the artifact
//! and the session's settled feedback, so a reload shows what was just posted.

use std::io;

use ags_feedback::{parse_feedback_json, Session};
use tiny_http::{Method, Request, Response};

use super::http::{csp_header, ctype, location, no_cache, read_body, reply, reply_str};
use super::{route, Route, Served};

pub(super) fn respond(mut request: Request, doc: &Served, session: &Session) -> io::Result<()> {
    let is_post = *request.method() == Method::Post;
    match route(request.url()) {
        Route::Feedback if is_post => {
            let body = read_body(&mut request);
            // A form post and a scripted one both land here. The form wants to go
            // back to where the reviewer was; a script wants a status it can read.
            if let Some(item) = form_post(&request, &body) {
                let anchor = ags_render::anchor_for(&item.block_id);
                // A scripted post wants the card back so it can insert it without
                // navigating; a plain form post wants to be sent to the block it
                // commented on. Both record the same item — only the answer
                // differs, which is what keeps the page working with no script.
                let fragment = wants_fragment(&request).then(|| card_for(&item));
                session.append(&item)?;
                return match fragment {
                    Some(html) => reply_html(request, &html),
                    None => see_other(request, &anchor),
                };
            }
            let outcome = record_feedback(session, &body)?;
            reply(request, outcome, "application/json; charset=utf-8")
        }
        Route::Finish if is_post => {
            let outcome = finish(session)?;
            // Same three-way split as a comment: the page's own script wants the
            // finished notice to swap in, a plain form wants to be sent back to
            // the document, and a script of someone else's wants a status.
            if wants_fragment(&request) {
                return reply_html(request, &ags_render::ended_notice());
            }
            if is_form(&request) {
                return see_other(request, "/");
            }
            reply(request, outcome, "application/json; charset=utf-8")
        }
        Route::Page => {
            let (feedback, ended) = session.settled().unwrap_or_default();
            let html = ags_render::review(doc.md, doc.name, &feedback, ended);
            page_response(request, &html)
        }
        // The close beacon (`navigator.sendBeacon`); arming/grace live in `serve_loop`.
        Route::Shutdown if is_post => reply(
            request,
            (200, "{\"ok\":true}"),
            "application/json; charset=utf-8",
        ),
        Route::Feedback | Route::Finish | Route::Shutdown => reply(
            request,
            (405, "method not allowed"),
            "text/plain; charset=utf-8",
        ),
        // The settled state, for the viewer to recover prior replies on load.
        Route::State => reply_str(
            request,
            &state_json(session, doc.name),
            "application/json; charset=utf-8",
        ),
        other => respond_get(request, doc.md, other),
    }
}

/// The settled review state as JSON (`{"feedback":[…],"ended":…}`) — the viewer
/// replays it on load so a re-opened review resumes where it left off.
pub(super) fn state_json(session: &Session, name: &str) -> String {
    let (feedback, ended) = session.settled().unwrap_or_default();
    serde_json::json!({ "feedback": feedback, "ended": ended, "name": name }).to_string()
}

/// Serve the static assets and the artifact for a GET route.
///
/// `Route::Page` never reaches here — the page is rendered in [`respond`] rather
/// than served as a file — so it falls to the catch-all below along with anything
/// unknown, which is the honest answer for a route this function does not own.
pub(super) fn respond_get(request: Request, artifact_md: &str, route: Route) -> io::Result<()> {
    match route {
        Route::Artifact => reply_str(request, artifact_md, "text/markdown; charset=utf-8"),
        // `Page` is answered in `respond` and never arrives here; it sits with the
        // rest rather than in an arm of its own, because an arm nothing can reach
        // is a branch nothing can test — and a 404 is the honest answer for a route
        // this function does not own.
        Route::Page
        | Route::Feedback
        | Route::Finish
        | Route::Shutdown
        | Route::State
        | Route::NotFound => {
            request.respond(Response::from_string("not found").with_status_code(404))
        }
    }
}

/// Parse one submitted feedback item and append it to the session. A malformed
/// body is a 400; a well-formed one is queued and returns 200. A store write
/// error propagates (the loop logs it and `tiny_http` answers 500).
pub(super) fn record_feedback(session: &Session, body: &str) -> io::Result<(u16, &'static str)> {
    match parse_feedback_json(body) {
        Ok(item) => {
            session.append(&item)?;
            Ok((200, "{\"ok\":true}"))
        }
        Err(_) => Ok((400, "{\"ok\":false,\"error\":\"bad-feedback\"}")),
    }
}

/// Finalize the review: mark the session ended so the next `poll` returns
/// `ended: true` after delivering any queued feedback.
pub(super) fn finish(session: &Session) -> io::Result<(u16, &'static str)> {
    session.finalize()?;
    Ok((200, "{\"ok\":true,\"ended\":true}"))
}

/// Whether a `Content-Type` names an HTML form rather than JSON.
///
/// Split from the request so the rule is testable without one: a browser appends
/// a charset (`…urlencoded; charset=UTF-8`), so this is a prefix match rather than
/// an equality, and that is the part worth pinning.
pub(super) fn is_form_type(content_type: &str) -> bool {
    content_type
        .trim_start()
        .starts_with("application/x-www-form-urlencoded")
}

/// Whether the request carries an HTML form rather than JSON.
pub(super) fn is_form(request: &Request) -> bool {
    request
        .headers()
        .iter()
        .any(|h| h.field.equiv("Content-Type") && is_form_type(h.value.as_str()))
}

/// The item a form post carries, or `None` when this was not a form post.
pub(super) fn form_post(request: &Request, body: &str) -> Option<ags_feedback::FeedbackItem> {
    is_form(request)
        .then(|| ags_feedback::parse_feedback_form(body).ok())
        .flatten()
}

/// Whether the caller asked for a rendered fragment rather than a redirect.
///
/// Set by the page's own script. A header rather than a query parameter, so the
/// posted body and the URL are identical on both paths and only the response
/// differs.
pub(super) fn wants_fragment(request: &Request) -> bool {
    request
        .headers()
        .iter()
        .any(|h| h.field.equiv("X-Ags-Fragment"))
}

/// The markup a scripted post gets back: the card for what was just recorded, or
/// nothing when the item is not something the page draws as a card.
pub(super) fn card_for(item: &ags_feedback::FeedbackItem) -> String {
    if item.kind == ags_feedback::FeedbackKind::Annotation {
        ags_render::note_card(item)
    } else {
        String::new()
    }
}

/// Answer with a markup fragment for the page to insert.
pub(super) fn reply_html(request: Request, html: &str) -> io::Result<()> {
    request.respond(
        Response::from_string(html)
            .with_header(ctype("text/html; charset=utf-8"))
            .with_header(no_cache()),
    )
}

/// Send the browser back where it came from.
///
/// `303` rather than `302`: it turns the follow-up into a GET, so a reload after
/// commenting re-reads the page instead of offering to post the comment again.
pub(super) fn see_other(request: Request, to: &str) -> io::Result<()> {
    request.respond(
        Response::empty(303)
            .with_header(location(to))
            .with_header(no_cache()),
    )
}

/// Answer with a rendered page.
///
/// Never cached: the page carries the review state at the moment it was built, so
/// a stale copy would show a comment as missing that the log already holds.
pub(super) fn page_response(request: Request, html: &str) -> io::Result<()> {
    request.respond(
        Response::from_string(html)
            .with_header(ctype("text/html; charset=utf-8"))
            .with_header(csp_header())
            .with_header(no_cache()),
    )
}
