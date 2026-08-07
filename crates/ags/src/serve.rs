//! Serve a validated artifact for in-browser review (inbrowser §1.2 / §3).
//!
//! A tiny local HTTP server: it renders the review page, records what the reviewer
//! says, and stops when the page is gone.
//!
//! Nothing is served as a file. The page is built per request from the artifact and
//! the session's settled feedback, so a reload shows what was just posted — and the
//! binary embeds no web assets at all.
//!
//! The host never runs JavaScript. What the page carries is written in `ags-render`
//! and rides under a nonce the CSP names; everything else a reviewer does is a form.

use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use ags_feedback::Session;
use tiny_http::Server;

mod http;
mod reply;
mod session;

use http::open_browser;
use session::{apply_request, done, findings_notice, prepare_session, record_findings, Presence};

/// How often the serve loop wakes to check a pending shutdown deadline.
const TICK: Duration = Duration::from_millis(250);

/// What a request path asks for.
///
/// There are no asset routes. The page is rendered per request rather than served
/// as a file, and nothing it carries has to be fetched — so `/viewer.js`,
/// `/viewer.css` and `/index.html` are gone, and with them the embedded bundle,
/// the brotli decompressor and the content negotiation that chose between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Route {
    Page,
    /// The artifact source, for anything that wants the markdown rather than the
    /// rendering of it.
    Artifact,
    Feedback,
    Finish,
    Shutdown,
    /// The settled review state — and, incidentally, the page's heartbeat.
    State,
    NotFound,
}

/// Map a request path (query string ignored) to a [`Route`].
pub(super) fn route(path: &str) -> Route {
    match path.split('?').next().unwrap_or(path) {
        "/" | "/index.html" => Route::Page,
        "/artifact.md" => Route::Artifact,
        "/feedback" => Route::Feedback,
        "/finish" => Route::Finish,
        "/shutdown" => Route::Shutdown,
        "/state" => Route::State,
        _ => Route::NotFound,
    }
}

/// What is being served: the artifact source, plus the name the viewer shows in its
/// bottom bar so a reviewer can tell which file they are looking at.
///
/// Carried as one value rather than as two parallel parameters, so the request chain
/// (`serve_loop` → `apply_request` → `respond`) threads a single argument.
pub(crate) struct Served<'a> {
    /// The artifact markdown.
    pub md: &'a str,
    /// Display name — the file name, not the full path.
    pub name: &'a str,
}

/// The file name of `path` for display, falling back to the whole path when it has
/// no final component (so the bar is never blank).
pub(crate) fn display_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into(),
    )
}

/// Serve `artifact_md` for review and open the browser. Blocks until the process
/// is killed. Port precedence: `port` arg → `$AGS_PORT` → an OS-assigned random
/// free port. Feedback the browser posts is appended to `artifact_path`'s session;
/// `fresh` discards any existing log first so the review starts from scratch, and a
/// non-`fresh` present reopens a previously-finished review for a new pass (see
/// [`prepare_session`]).
///
/// # Errors
/// Propagates the server bind error and the feedback-session open/reset/reopen error.
pub fn serve(
    artifact_path: &Path,
    artifact_md: &str,
    port: Option<u16>,
    fresh: bool,
) -> io::Result<()> {
    let port = port
        .or_else(|| std::env::var("AGS_PORT").ok().and_then(|p| p.parse().ok()))
        .unwrap_or(0);
    let server = Server::http(("127.0.0.1", port)).map_err(io::Error::other)?;
    let session = crate::store::open_session(artifact_path)?;
    prepare_session(&session, fresh)?;
    let findings = record_findings(&session, artifact_md)?;
    let addr = server.server_addr().to_ip();
    let host = addr.map_or_else(|| "127.0.0.1:0".to_string(), |a| a.to_string());
    let bound_port = addr.map_or(0, |a| a.port());
    let url = format!("http://{host}");
    let pid = std::process::id();
    // The log is the instance registry (pid/port) — no pidfile.
    session.record_serve(pid, bound_port, &url)?;
    println!("ags: serving artifact for review at {url} (ctrl-c to stop)");
    // An `Option` iterated rather than tested. Clippy prefers `if let`, and would be
    // right in most places; here the branch is the point being avoided. `serve` binds
    // a socket and opens a browser, so no test reaches this line, and an `if let` puts
    // an arm here that nothing can cover — which is what dropped this function under
    // the coverage floor when it was written that way. The decision lives in
    // `findings_notice`, which a test does call.
    #[expect(
        for_loops_over_fallibles,
        reason = "an if-let arm here is uncoverable: serve binds a socket, so no test reaches it"
    )]
    for line in findings_notice(findings) {
        println!("ags: {line}");
    }
    open_browser(&url);
    let doc = Served {
        md: artifact_md,
        name: &display_name(artifact_path),
    };
    serve_loop(&server, &doc, &session);
    // Record the stop, tagged with whether the review was finished (completed) or the
    // reviewer left first (abandoned) — so a poll can tell the two apart.
    let ended = session.settled().is_ok_and(|(_, ended)| ended);
    if let Err(err) = session.record_shutdown(pid, ended) {
        eprintln!("ags: could not record shutdown: {err}");
    }
    Ok(())
}

/// The request loop. Runs until a `/shutdown` grace window elapses without the page
/// reconnecting, or a test bound (`$AGS_SERVE_MAX_REQUESTS`) is reached. Uses
/// `recv_timeout`, not the blocking `incoming_requests` iterator, so the loop can wake on
/// a tick to fire a pending shutdown even though a closed tab sends no further requests.
fn serve_loop(server: &Server, doc: &Served, session: &Session) {
    let max = std::env::var("AGS_SERVE_MAX_REQUESTS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok());
    let grace = Duration::from_millis(
        std::env::var("AGS_SHUTDOWN_GRACE_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(10_000),
    );
    let mut pending: Option<Instant> = None;
    let mut seen = Presence::default();
    let mut count = 0usize;
    loop {
        if let Ok(Some(request)) = server.recv_timeout(TICK) {
            seen.at(route(request.url()), Instant::now());
            pending = apply_request(request, doc, session, grace);
            count += 1;
            if max.is_some_and(|m| count >= m) {
                break;
            }
        } else if let Some(why) = done(pending, &seen, Instant::now()) {
            // A tick with no request, or a transient recv error.
            println!("ags: {why} — stopping.");
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::http::{csp_header, location, no_cache, open_browser_cmd, CSP};
    use super::reply::{finish, is_form_type, record_feedback, respond, state_json};
    use super::session::{findings_notice, next_deadline, record_findings, IDLE};
    use super::*;
    use std::fs;

    /// A fresh feedback session backed by a temp log file.
    fn temp_session(name: &str) -> Session {
        let dir = std::env::temp_dir().join("ags-serve-test").join(name);
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        Session::open_at(dir.join("artifact.md.ags.jsonl")).unwrap()
    }

    #[test]
    fn routes_map_paths() {
        assert_eq!(route("/"), Route::Page);
        assert_eq!(route("/index.html"), Route::Page);
        assert_eq!(route("/artifact.md"), Route::Artifact);
        assert_eq!(route("/artifact.md?v=1"), Route::Artifact);
        assert_eq!(route("/feedback"), Route::Feedback);
        assert_eq!(route("/finish"), Route::Finish);
        assert_eq!(route("/shutdown"), Route::Shutdown);
        assert_eq!(route("/state"), Route::State);
        assert_eq!(route("/favicon.ico"), Route::NotFound);
    }

    /// Drive one request through `respond` against a real server, and hand back
    /// the raw response.
    ///
    /// A `tiny_http::Request` can only come from a `Server`, so the response path
    /// is unreachable without binding one. Everything here is in-process and on an
    /// OS-assigned port, so it needs no fixture.
    ///
    /// `who` names this caller's session directory. It is not decoration: the
    /// helper wipes and recreates that directory, so sharing one name across these
    /// tests made them destroy each other's log whenever they ran in parallel.
    fn round_trip(who: &str, method: &str, path: &str, content_type: &str, body: &str) -> String {
        round_trip_with(who, method, path, content_type, body, &[])
    }

    /// As [`round_trip`], with extra request headers.
    fn round_trip_with(
        who: &str,
        method: &str,
        path: &str,
        content_type: &str,
        body: &str,
        extra: &[(&str, &str)],
    ) -> String {
        use std::io::Write as _;
        let server = Server::http("127.0.0.1:0").expect("bind a test server");
        let port = server.server_addr().to_ip().expect("an ip address").port();
        let (method, path) = (method.to_string(), path.to_string());
        let (content_type, body) = (content_type.to_string(), body.to_string());
        let extra: String = extra
            .iter()
            .map(|(k, v)| format!("{k}: {v}\r\n"))
            .collect::<Vec<_>>()
            .concat();
        let client = std::thread::spawn(move || {
            let mut socket = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
            write!(
                socket,
                "{method} {path} HTTP/1.1\r\nHost: t\r\nContent-Type: {content_type}\r\n\
                 {extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .expect("write the request");
            let mut out = String::new();
            std::io::Read::read_to_string(&mut socket, &mut out).expect("read the response");
            out
        });
        let request = server.recv().expect("receive the request");
        let session = temp_session(who);
        let doc = Served {
            md: "# T\n\n```mermaid #flow\ngraph TD\n  A --> B\n```\n",
            name: "t.md",
        };
        respond(request, &doc, &session).expect("respond");
        client.join().expect("the client thread")
    }

    #[test]
    fn the_page_arrives_rendered_rather_than_as_a_bundle_to_run() {
        let out = round_trip("page", "GET", "/", "text/plain", "");
        assert!(out.contains("200 OK"), "{out}");
        assert!(
            out.contains("<svg"),
            "the diagram is drawn into the page: {out}"
        );
        assert!(!out.contains("viewer.js"), "{out}");
        assert!(out.contains("action=\"/feedback\""), "{out}");
    }

    #[test]
    fn a_form_post_is_recorded_and_sent_back_to_the_block() {
        // The whole no-script path in one exchange: a comment posts as a form and
        // the answer is a redirect to where the reviewer was standing.
        let out = round_trip(
            "form-post",
            "POST",
            "/feedback",
            "application/x-www-form-urlencoded",
            "block_id=flow&sub=A&kind=annotation&body=why+here",
        );
        assert!(out.contains("303 See Other"), "{out}");
        assert!(out.contains("Location: /#block-flow"), "{out}");
    }

    #[test]
    fn a_scripted_form_post_gets_the_card_back_instead_of_a_redirect() {
        // The no-reload path: the page inserts what comes back rather than
        // navigating, so the reviewer keeps their scroll position.
        let out = round_trip_with(
            "fragment",
            "POST",
            "/feedback",
            "application/x-www-form-urlencoded",
            "block_id=flow&sub=A&kind=annotation&body=stays+put",
            &[("X-Ags-Fragment", "1")],
        );
        assert!(out.contains("200 OK"), "{out}");
        assert!(out.contains("note-card"), "{out}");
        assert!(out.contains("stays put"), "{out}");
        assert!(!out.contains("303"), "{out}");
    }

    #[test]
    fn an_answer_records_but_has_no_card_to_insert() {
        let out = round_trip_with(
            "fragment-answer",
            "POST",
            "/feedback",
            "application/x-www-form-urlencoded",
            "block_id=q1&kind=answer&body=SQLite",
            &[("X-Ags-Fragment", "1")],
        );
        assert!(out.contains("200 OK"), "{out}");
        assert!(!out.contains("note-card"), "{out}");
    }

    #[test]
    fn a_scripted_finish_gets_the_notice_back_instead_of_a_redirect() {
        // Finishing used to navigate, and a reload threw away where the reviewer
        // was standing — the same fault a comment had, one control later.
        let out = round_trip_with(
            "finish-fragment",
            "POST",
            "/finish",
            "application/x-www-form-urlencoded",
            "",
            &[("X-Ags-Fragment", "1")],
        );
        assert!(out.contains("200 OK"), "{out}");
        assert!(out.contains("This review is finished"), "{out}");
        assert!(!out.contains("303"), "{out}");
    }

    #[test]
    fn a_plain_form_finish_is_still_sent_back_to_the_document() {
        // The no-script path is unchanged; only the answer to a scripted caller is.
        let out = round_trip_with(
            "finish-form",
            "POST",
            "/finish",
            "application/x-www-form-urlencoded",
            "",
            &[],
        );
        assert!(out.contains("303 See Other"), "{out}");
        assert!(out.contains("Location: /"), "{out}");
    }

    #[test]
    fn a_scripted_post_still_gets_a_status_it_can_read() {
        // The JSON path has to keep working: a redirect is no use to a fetch.
        let out = round_trip(
            "json-post",
            "POST",
            "/feedback",
            "application/json",
            "{\"block_id\":\"flow\",\"kind\":\"annotation\",\"body\":\"x\"}",
        );
        assert!(out.contains("200 OK"), "{out}");
        assert!(out.contains("{\"ok\":true}"), "{out}");
    }

    #[test]
    fn an_unknown_path_is_a_404() {
        assert!(round_trip("not-found", "GET", "/nope", "text/plain", "").contains("404"));
    }

    #[test]
    fn the_artifact_is_still_served_for_anything_that_wants_the_source() {
        let out = round_trip("artifact", "GET", "/artifact.md", "text/plain", "");
        assert!(out.contains("graph TD"), "{out}");
    }

    /// A moment either side of the idle window, for the tests below.
    const SLACK: Duration = Duration::from_secs(1);

    #[test]
    fn the_loop_stops_for_a_stated_reason_or_not_at_all() {
        let now = Instant::now();
        let mut beating = Presence::default();
        beating.at(Route::State, now);
        // Still here: heartbeating, nothing pending.
        assert_eq!(done(None, &beating, now), None);
        // The beacon's window elapsed.
        assert_eq!(
            done(Some(now), &Presence::default(), now),
            Some("the page closed and did not come back")
        );
        // The heartbeat stopped.
        assert_eq!(
            done(None, &beating, now + IDLE + SLACK),
            Some("the page stopped answering")
        );
        // A page that never heartbeated and has no beacon pending is left alone.
        assert_eq!(done(None, &Presence::default(), now + IDLE + SLACK), None);
    }

    #[test]
    fn a_page_that_never_heartbeats_is_never_timed_out() {
        // A page with no script cannot promise a heartbeat, so it must not be
        // judged by one. The beacon stays its only signal.
        let mut seen = Presence::default();
        let start = Instant::now();
        seen.at(Route::Page, start);
        seen.at(Route::Feedback, start);
        assert!(!seen.gone(start + IDLE + Duration::from_mins(1)));
    }

    #[test]
    fn a_page_that_stops_heartbeating_is_gone() {
        let mut seen = Presence::default();
        let start = Instant::now();
        seen.at(Route::State, start);
        assert!(!seen.gone(start + IDLE.saturating_sub(SLACK)));
        assert!(seen.gone(start + IDLE + SLACK));
    }

    #[test]
    fn any_request_from_a_heartbeating_page_keeps_it_alive() {
        // Once the clock is running, everything counts — a reviewer typing a
        // comment is at least as present as a timer firing.
        let mut seen = Presence::default();
        let start = Instant::now();
        seen.at(Route::State, start);
        let later = start + IDLE.saturating_sub(SLACK);
        seen.at(Route::Feedback, later);
        assert!(!seen.gone(later + IDLE.saturating_sub(SLACK)));
    }

    #[test]
    fn a_page_load_cancels_a_pending_shutdown() {
        let grace = Duration::from_secs(2);
        // The beacon arms it ...
        assert!(next_deadline(Route::Shutdown, true, grace).is_some());
        // ... and the page reconnecting says the tab did not close after all.
        assert!(next_deadline(Route::Page, false, grace).is_none());
    }

    #[test]
    fn anything_arriving_after_the_beacon_cancels_it() {
        // A request is proof the page is still there, whatever it asked for. The
        // rule used to cancel only on a page load, which made a reload's survival
        // depend on whether the beacon or the reload reached the loop first.
        let grace = Duration::from_secs(2);
        assert!(next_deadline(Route::Shutdown, true, grace).is_some());
        for route in [
            Route::Page,
            Route::Feedback,
            Route::State,
            Route::Artifact,
            Route::NotFound,
        ] {
            assert!(
                next_deadline(route, false, grace).is_none(),
                "{route:?} left the shutdown armed"
            );
        }
    }

    #[test]
    fn a_get_to_shutdown_does_not_arm_it() {
        // Only the beacon, which is a POST. A stray GET must not end a review.
        let grace = Duration::from_secs(2);
        assert_eq!(next_deadline(Route::Shutdown, false, grace), None);
    }

    #[test]
    fn a_form_content_type_is_recognised_with_or_without_a_charset() {
        assert!(is_form_type("application/x-www-form-urlencoded"));
        assert!(is_form_type(
            "application/x-www-form-urlencoded; charset=UTF-8"
        ));
        assert!(is_form_type(" application/x-www-form-urlencoded"));
        // A scripted post is JSON and must not be read as a form.
        assert!(!is_form_type("application/json"));
        assert!(!is_form_type("text/plain"));
        assert!(!is_form_type(""));
    }

    #[test]
    fn a_redirect_names_where_it_is_going() {
        let header = location("/#block-flow");
        assert!(header.field.equiv("Location"));
        assert_eq!(header.value.as_str(), "/#block-flow");
    }

    #[test]
    fn csp_locks_script_execution_to_the_pages_own_script() {
        // The served CSP is the real HTML-safety boundary. A nonce is tighter than
        // the `'self'` it replaced: that would run any same-origin file, this runs
        // only the one script the page itself wrote.
        assert!(CSP.contains("script-src 'nonce-ags-review'"));
        assert!(
            !CSP.contains("'unsafe-inline'") || !CSP.contains("script-src 'nonce-ags-review' 'un"),
            "script must not allow blanket inline execution"
        );
        assert!(CSP.contains("object-src 'none'"));
        assert!(CSP.contains("base-uri 'none'"));
        // Themed content + the diagram SVG need inline styles (which can't run code).
        assert!(CSP.contains("style-src 'self' 'unsafe-inline'"));
        // A comment and an answer post to this origin with no script at all.
        assert!(CSP.contains("form-action 'self'"));
        let _ = csp_header();
    }

    #[test]
    fn assets_are_served_no_store() {
        // The dev-serve rebuilds the bundle each run with no cache-busting in the URL,
        // so assets must not be cached — a stale viewer.js/.css against a newer artifact
        // renders as broken styling.
        let h = no_cache();
        assert!(h
            .field
            .as_str()
            .as_str()
            .eq_ignore_ascii_case("cache-control"));
        assert_eq!(h.value.as_str(), "no-store");
    }

    #[test]
    fn records_valid_feedback_and_rejects_malformed() {
        let session = temp_session("record");
        let good = r#"{"block_id":"flow","sub_target":{"Node":"Auth"},"kind":"annotation","body":"wrong way"}"#;
        assert_eq!(record_feedback(&session, good).unwrap().0, 200);
        assert_eq!(record_feedback(&session, "garbage").unwrap().0, 400);
        let (items, _) = session.settled().unwrap();
        assert_eq!(items.len(), 1, "only the valid item is recorded");
        assert_eq!(items[0].anchor(), "#flow/node:Auth");
    }

    #[test]
    fn finish_finalizes_the_session() {
        let session = temp_session("finish");
        assert!(!session.settled().unwrap().1);
        assert_eq!(finish(&session).unwrap().0, 200);
        assert!(session.settled().unwrap().1);
    }

    #[test]
    fn prepare_reopens_a_finished_review_when_not_fresh() {
        // Presenting a finished artifact again opens a new pass and keeps the feedback.
        let session = temp_session("prepare-reopen");
        record_feedback(
            &session,
            r#"{"block_id":"q","kind":"answer","body":"SQLite"}"#,
        )
        .unwrap();
        finish(&session).unwrap();
        assert!(session.settled().unwrap().1, "review starts ended");
        prepare_session(&session, false).unwrap();
        let (items, ended) = session.settled().unwrap();
        assert!(!ended, "a non-fresh present reopens the finished review");
        assert_eq!(items.len(), 1, "prior feedback is kept");
    }

    #[test]
    fn prepare_fresh_discards_the_log() {
        let session = temp_session("prepare-fresh");
        record_feedback(
            &session,
            r#"{"block_id":"q","kind":"answer","body":"SQLite"}"#,
        )
        .unwrap();
        prepare_session(&session, true).unwrap();
        assert!(
            session.settled().unwrap().0.is_empty(),
            "--fresh wipes the log"
        );
    }

    #[test]
    fn prepare_leaves_an_open_review_untouched() {
        let session = temp_session("prepare-open");
        record_feedback(
            &session,
            r#"{"block_id":"q","kind":"answer","body":"SQLite"}"#,
        )
        .unwrap();
        prepare_session(&session, false).unwrap();
        let (items, ended) = session.settled().unwrap();
        assert!(!ended, "an open review stays open");
        assert_eq!(items.len(), 1, "and its feedback is unchanged");
    }

    #[test]
    fn state_json_reports_settled_feedback() {
        let session = temp_session("state");
        record_feedback(
            &session,
            r#"{"block_id":"q","kind":"answer","body":"SQLite"}"#,
        )
        .unwrap();
        let json = state_json(&session, "artifact.md");
        assert!(
            json.contains("\"feedback\"") && json.contains("SQLite"),
            "{json}"
        );
        assert!(json.contains("\"ended\":false"), "{json}");
        // The viewer's bottom bar reads the name from here.
        assert!(json.contains("\"name\":\"artifact.md\""), "{json}");
        finish(&session).unwrap();
        assert!(state_json(&session, "artifact.md").contains("\"ended\":true"));
    }

    /// A diagram that draws but reads wrong: every node on one row joined to
    /// every node on the next. In a layered drawing every pair of edges whose
    /// endpoints invert has to cross, and with three by three that is nine — in
    /// any order the rows are put in, so no later work on the layout can quietly
    /// take this fixture away.
    ///
    /// It has been a real gallery diagram twice: the `ci` subgraph enclosing a
    /// node not in it, then a state machine whose transitions crossed. The
    /// engine fixed both, which is a good problem to have and a poor way to keep
    /// a fixture.
    const READS_WRONG: &str = "```mermaid #p\ngraph TD\n  A1 --> B1\n  A1 --> B2\n  A1 --> B3\n  A2 --> B1\n  A2 --> B2\n  A2 --> B3\n  A3 --> B1\n  A3 --> B2\n  A3 --> B3\n```\n";

    #[test]
    fn a_legibility_finding_is_recorded_once_and_retired_when_fixed() {
        let session = temp_session("findings");

        assert_eq!(record_findings(&session, READS_WRONG).unwrap(), 1);
        let (items, _) = session.settled().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].kind, ags_feedback::FeedbackKind::Finding);
        assert_eq!(items[0].block_id, "p");

        // Presenting the unchanged artifact again neither duplicates the finding
        // nor writes anything, because nothing about the drawing changed.
        let before = fs::read_to_string(session.path()).unwrap();
        assert_eq!(record_findings(&session, READS_WRONG).unwrap(), 1);
        assert_eq!(
            fs::read_to_string(session.path()).unwrap(),
            before,
            "an unchanged artifact should leave the log alone"
        );

        // The agent redraws it. The finding is derived, so it retires itself —
        // nothing else ever would, since the log only grows.
        assert_eq!(
            record_findings(&session, "```mermaid #p\ngraph TD\n  A-->B\n```\n").unwrap(),
            0
        );
        assert_eq!(session.settled().unwrap().0, vec![]);
    }

    #[test]
    fn the_findings_notice_says_nothing_when_there_is_nothing_to_say() {
        assert_eq!(findings_notice(0), None);
        assert!(findings_notice(1).unwrap().contains("1 diagram reported"));
        assert!(findings_notice(7).unwrap().contains("7 diagrams reported"));
    }

    #[test]
    fn display_name_is_the_file_name_and_falls_back_to_the_path() {
        assert_eq!(
            display_name(Path::new("/tmp/notes/artifact.md")),
            "artifact.md"
        );
        // A path with no final component — the bar shows the path rather than nothing.
        assert_eq!(display_name(Path::new("/")), "/");
        assert_eq!(display_name(Path::new("..")), "..");
    }

    #[test]
    fn open_browser_cmd_is_best_effort() {
        // `true` spawns and exits; a missing binary hits the graceful-failure arm.
        open_browser_cmd("true", "http://127.0.0.1:1");
        open_browser_cmd("/no/such/binary/ags-xyzzy", "http://127.0.0.1:1");
    }
}
