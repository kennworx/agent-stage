//! When the review is over, and what the session records while it runs.
//!
//! The page is gone when nothing has polled for a while — there is no logout, so
//! presence is inferred from whether anyone is still asking.

use std::io;
use std::time::{Duration, Instant};

use ags_feedback::Session;

use tiny_http::{Method, Request};

use super::reply::respond;
use super::{route, Route, Served};

pub(super) fn done(
    pending: Option<Instant>,
    seen: &Presence,
    now: Instant,
) -> Option<&'static str> {
    if pending.is_some_and(|deadline| now >= deadline) {
        return Some("the page closed and did not come back");
    }
    seen.gone(now).then_some("the page stopped answering")
}

/// How long a heartbeating page may go quiet before it is treated as gone.
///
/// Comfortably more than the page's own interval, so a slow tab, a sleeping
/// laptop waking, or a reload does not read as a departure.
pub(super) const IDLE: Duration = Duration::from_secs(25);

/// Whether the page is still there.
///
/// The close beacon is best-effort *notice of absence*: it is a fire-and-forget
/// request sent while the tab is being torn down, and treating it as proof ended
/// reviews that were still in use. A heartbeat is the opposite — positive evidence
/// of presence, arriving on a schedule the host can reason about.
///
/// Only pages that have heartbeated at least once are held to it. A page with no
/// script never will, and must not be timed out for a promise it never made; for
/// those the beacon is still the only signal there is.
#[derive(Debug, Default)]
pub(super) struct Presence {
    last: Option<Instant>,
}

impl Presence {
    /// Note a request. Any request is evidence, but only a heartbeat starts the
    /// clock — a page that never sends one is never judged by it.
    pub(super) fn at(&mut self, route: Route, now: Instant) {
        if route == Route::State || self.last.is_some() {
            self.last = Some(now);
        }
    }

    /// Whether a page that was heartbeating has stopped.
    pub(super) fn gone(&self, now: Instant) -> bool {
        self.last
            .is_some_and(|last| now.duration_since(last) > IDLE)
    }
}

/// Handle one request and return the next pending-shutdown deadline. A POST `/shutdown`
/// arms the grace window (the close beacon); a full page load cancels it (a reload
/// reconnecting); every other request leaves it unchanged, so background `/state` polls
/// and the beacon itself never cancel a genuine close.
pub(super) fn apply_request(
    request: Request,
    doc: &Served,
    session: &Session,
    grace: Duration,
) -> Option<Instant> {
    let is_post = *request.method() == Method::Post;
    let next = next_deadline(route(request.url()), is_post, grace);
    if let Err(err) = respond(request, doc, session) {
        eprintln!("ags: response error: {err}");
    }
    next
}

/// The shutdown deadline after one request.
///
/// A POST `/shutdown` is the close beacon and arms the grace window; anything at
/// all afterwards cancels it, because a request is proof the page is still there.
///
/// It takes no previous deadline because it needs none: every request either arms
/// or clears, so the answer depends on this request alone. That is the whole of
/// the fix — the old rule carried `pending` forward for anything that was not a
/// page load, which is what let a beacon survive the reload that disproved it.
pub(super) fn next_deadline(route: Route, is_post: bool, grace: Duration) -> Option<Instant> {
    match route {
        Route::Shutdown if is_post => Some(Instant::now() + grace),
        // *Any* other request cancels, not just a page load. If the tab is really
        // gone nothing else arrives; if anything does, the reviewer is still here.
        // Cancelling only on `Route::Page` made survival depend on whether the
        // beacon or the reload that follows it reached this loop first — a race
        // the browser decides, which ended reviews at random.
        _ => None,
    }
}

/// Ready the session for this presentation. `--fresh` discards the log for a clean
/// slate; otherwise, if the review was previously finished, reopen it so this present
/// is a new review pass — presenting an artifact *is* a request to review it — while
/// keeping the prior feedback as history. An already-open review is left untouched.
///
/// # Errors
/// Propagates the session reset/settled/reopen I/O error.
pub(super) fn prepare_session(session: &Session, fresh: bool) -> io::Result<()> {
    if fresh {
        session.reset()
    } else if session.settled()?.1 {
        session.reopen()
    } else {
        Ok(())
    }
}

/// Record what the drawings get wrong, so `ags poll` hands it back with the rest.
///
/// Once per presentation rather than per request: a finding is a property of the
/// artifact, not of a page view, and the page is rendered fresh every time — so
/// recording it in the render path would append the same finding on every reload.
///
/// Findings are derived, so they are also retracted here. A diagram the agent
/// redrew reports nothing this time, and [`ags_render::finding_updates`] turns its
/// stale record into a delete; nothing else would ever retire it, because the log
/// only grows.
///
/// What to tell the reviewer about findings, or `None` when there is nothing to say.
///
/// Said out loud rather than only written to the log: the reviewer is about to look
/// at these drawings, and the agent will not see the log until it polls.
pub(super) fn findings_notice(count: usize) -> Option<String> {
    (count > 0).then(|| {
        let diagrams = if count == 1 { "diagram" } else { "diagrams" };
        format!("{count} {diagrams} reported legibility findings — sent to the agent")
    })
}

/// Returns how many diagrams are *currently* reporting something, which is what the
/// reviewer is told — not how many lines were written. Those differ: a redraw that
/// fixed a diagram writes a line to retire its finding, and announcing that as a
/// finding would report a problem at the moment it was solved.
///
/// # Errors
/// Propagates the session read and append errors.
pub(super) fn record_findings(session: &Session, artifact_md: &str) -> io::Result<usize> {
    let recorded = session.settled()?.0;
    let current = ags_render::render_findings(artifact_md);
    let reporting = current.len();
    for item in &ags_render::finding_updates(current, &recorded) {
        session.append(item)?;
    }
    Ok(reporting)
}
