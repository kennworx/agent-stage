//! The long-poll mechanic: read the session's settled state, and if nothing has
//! been submitted yet, wait (emitting heartbeats) until feedback arrives, the
//! review ends, or a bound is reached. `settled` is a pure replay of the log, so a
//! repeated or interrupted poll returns the same snapshot — it never consumes.
//!
//! The wait is bounded and its timing is injected, so it is deterministic to test
//! and the CLI supplies production values (a 1-tick-per-second heartbeat).

use std::io;
use std::time::Duration;

use crate::model::FeedbackItem;
use crate::store::Session;
use crate::wire::poll_to_toon;

/// Poll `session`: return the settled feedback + the review state as TOON. Returns
/// immediately if any feedback exists, the review has ended, or the review is closed
/// (its serving instance is gone); otherwise waits up to `max_ticks` intervals, calling
/// `heartbeat` each tick, then returns whatever has settled. Idempotent — the same
/// snapshot on every call. A `closed` result stops the wait so a reviewer who left
/// without finishing never blocks the agent forever.
///
/// `anchoring` is called only when a response is about to be built, and yields the
/// test for whether an item's anchor still resolves. A poll blocks for minutes and
/// the agent is free to rewrite the artifact while it waits, so building the test
/// on entry would measure detachment against a version that no longer exists.
///
/// A function returning a function, rather than the artifact text: this crate has
/// no idea what an artifact is, and asking for one would put a renderer in its
/// dependency list to answer a question its caller already knows the answer to.
///
/// # Errors
/// Propagates I/O errors from reading the session log.
pub fn poll_blocking<Test: Fn(&FeedbackItem) -> bool>(
    session: &Session,
    interval: Duration,
    max_ticks: u32,
    mut heartbeat: impl FnMut(),
    anchoring: impl Fn() -> Test,
) -> io::Result<String> {
    for _ in 0..max_ticks {
        let (items, ended, closed) = session.outcome(process_is_alive)?;
        if !items.is_empty() || ended || closed {
            return Ok(poll_to_toon(&items, ended, closed, anchoring()));
        }
        heartbeat();
        std::thread::sleep(interval);
    }
    let (items, ended, closed) = session.outcome(process_is_alive)?;
    Ok(poll_to_toon(&items, ended, closed, anchoring()))
}

/// Whether the process `pid` is currently alive. Used to resolve a review as `closed`
/// when its server died without recording a shutdown (crash / kill / a close beacon that
/// never fired). On Unix a `kill -0` probe answers it without `unsafe`; elsewhere we
/// conservatively assume alive, so shutdown detection falls back to the recorded
/// `shutdown` event.
#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Non-Unix fallback: assume the process is alive (graceful-shutdown detection still
/// works via the recorded `shutdown` event).
#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FeedbackItem, FeedbackKind};
    use std::fs;
    use std::path::{Path, PathBuf};

    fn root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("ags-poll-test").join(name);
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn session(root: &Path) -> Session {
        Session::open_at(root.join("artifact.md.ags.jsonl")).unwrap()
    }

    /// The two verdicts this crate can be handed. It does not reach either one —
    /// that needs a renderer — so the tests supply them directly.
    fn attached(_: &FeedbackItem) -> bool {
        false
    }
    fn detached_from_everything(_: &FeedbackItem) -> bool {
        true
    }

    #[test]
    fn returns_queued_items_immediately_without_ticking() {
        let r = root("immediate");
        let s = session(&r);
        s.append(&FeedbackItem::new("a", None, FeedbackKind::Annotation, "hi").unwrap())
            .unwrap();
        let mut beats = 0;
        let toon = poll_blocking(&s, Duration::ZERO, 5, || beats += 1, || attached).unwrap();
        assert_eq!(beats, 0, "should not wait when feedback is already queued");
        assert!(toon.contains("#a,annotation,hi"));
        assert!(toon.contains("ended: false") && toon.contains("closed: false"));
    }

    #[test]
    fn waits_the_bound_then_returns_empty() {
        let r = root("bound");
        let s = session(&r);
        let mut beats = 0;
        let toon = poll_blocking(&s, Duration::ZERO, 3, || beats += 1, || attached).unwrap();
        assert_eq!(beats, 3, "empty store should tick to the bound");
        // toon-rs renders an empty array without a column spec.
        assert_eq!(toon, "feedback[0]:\nended: false\nclosed: false");
    }

    #[test]
    fn ended_review_returns_immediately() {
        let r = root("ended");
        let s = session(&r);
        s.finalize().unwrap();
        let mut beats = 0;
        let toon = poll_blocking(&s, Duration::ZERO, 5, || beats += 1, || attached).unwrap();
        assert_eq!(beats, 0);
        assert!(toon.contains("ended: true"));
    }

    #[test]
    fn finish_delivers_queued_items_then_ended() {
        let r = root("finish");
        let s = session(&r);
        s.append(&FeedbackItem::new("q", None, FeedbackKind::Answer, "Rust").unwrap())
            .unwrap();
        s.finalize().unwrap();
        let toon = poll_blocking(&s, Duration::ZERO, 5, || {}, || attached).unwrap();
        assert!(toon.contains("#q,answer,Rust"), "queued item delivered");
        assert!(toon.contains("ended: true"), "and the ended status");
    }

    #[test]
    fn the_detached_column_follows_the_verdict_it_is_given() {
        // Same log, two verdicts, two answers — and the item is delivered either
        // way. Which verdict is right needs a renderer, and is tested where one
        // exists; what is tested here is that this crate reports what it is told
        // and never drops the item on account of it.
        let r = root("detached");
        let s = session(&r);
        s.append(
            &FeedbackItem::new(
                "flow",
                Some(crate::model::SubTarget::Node("Auth".into())),
                FeedbackKind::Annotation,
                "wrong arrow",
            )
            .unwrap(),
        )
        .unwrap();

        // The `:` in a node anchor is TOON-significant, so the field is quoted.
        let still_there = poll_blocking(&s, Duration::ZERO, 1, || {}, || attached).unwrap();
        assert!(
            still_there.contains("false,\"#flow/node:Auth\""),
            "{still_there}"
        );

        let gone =
            poll_blocking(&s, Duration::ZERO, 1, || {}, || detached_from_everything).unwrap();
        assert!(gone.contains("true,\"#flow/node:Auth\""), "{gone}");
        assert!(
            gone.contains("wrong arrow"),
            "and is still delivered: {gone}"
        );
    }

    // A pid far beyond any real process table — accepted by `kill` but never alive.
    const DEAD_PID: u32 = 1_999_999_999;

    #[test]
    fn a_dead_server_pid_closes_the_poll() {
        // The server crashed without recording a shutdown; the poll must not block.
        let r = root("closed");
        let s = session(&r);
        s.record_serve(DEAD_PID, 8770, "http://127.0.0.1:8770")
            .unwrap();
        let mut beats = 0;
        let toon = poll_blocking(&s, Duration::ZERO, 5, || beats += 1, || attached).unwrap();
        assert_eq!(beats, 0, "a closed review returns immediately");
        assert!(toon.contains("closed: true"), "{toon}");
    }

    #[cfg(unix)]
    #[test]
    fn process_liveness_detects_self_and_a_dead_pid() {
        assert!(process_is_alive(std::process::id()), "self is alive");
        assert!(!process_is_alive(DEAD_PID), "an unused pid is not alive");
    }

    #[cfg(not(unix))]
    #[test]
    fn process_liveness_is_conservative_off_unix() {
        assert!(process_is_alive(DEAD_PID));
    }
}
