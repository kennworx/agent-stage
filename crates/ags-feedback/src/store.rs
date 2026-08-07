//! Local, file-based review log — a single append-only JSONL beside the artifact.
//!
//! `<artifact>.ags.jsonl` holds one JSON object per line: each submitted feedback
//! item (annotation/answer/finding, with its `status`) in order, plus an
//! `{"ended":…}` marker when a review is finished (`true`) or reopened for another
//! pass (`false`) — the latest marker wins. The log is the audit trail; the
//! **settled state** is its replay — for each anchor the latest item wins and a
//! `delete` drops it — so re-opening recovers the reviewer's work and a poll
//! returns the current answer idempotently, with no cursor or pending/delivered
//! bookkeeping. The library takes the log path explicitly; where it lives (beside
//! the artifact, or a configured dir) is the CLI's concern.

use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{FeedbackItem, FeedbackStatus};

/// One line in the review log: a feedback item, an end-of-review marker, or a server
/// lifecycle event. Untagged, distinguished by shape — `Item` has `block_id`, `End` has
/// `ended`, `Life` has `event`.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum LogEntry {
    /// A submitted feedback item (its `block_id`/`kind`/`body` distinguish it).
    Item(FeedbackItem),
    /// A review-state marker (`{"ended":true}` finish / `{"ended":false}` reopen).
    End(EndMarker),
    /// A server lifecycle event (`serve`/`shutdown`).
    Life(Lifecycle),
}

/// A review-state marker line: `ended` is `true` for a finish, `false` for a reopen.
#[derive(Serialize, Deserialize)]
struct EndMarker {
    ended: bool,
}

/// A server lifecycle event, internally tagged by `event`. `serve` records the serving
/// instance (so the log is the registry — no pidfile); `shutdown` records its stop and
/// whether the review was completed. Both carry `pid` so a shutdown pairs with its serve
/// and a reader can liveness-check the process.
#[derive(Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
enum Lifecycle {
    Serve { pid: u32, port: u16, url: String },
    Shutdown { pid: u32, completed: bool },
}

/// One pass over the log: the settled items, the ended flag, the last serving pid, and
/// the pids that recorded a shutdown. [`Session::settled`] and [`Session::outcome`] both
/// read it, so the fold happens once.
#[derive(Default)]
struct Replay {
    items: Vec<FeedbackItem>,
    ended: bool,
    last_serve_pid: Option<u32>,
    shutdown_pids: Vec<u32>,
}

/// A review session backed by one append-only JSONL log file.
pub struct Session {
    path: PathBuf,
}

impl Session {
    /// Open the session whose log lives at `log_path`, creating parent dirs.
    ///
    /// # Errors
    /// Propagates I/O errors from creating the parent directory.
    pub fn open_at(log_path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = log_path.into();
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        Ok(Self { path })
    }

    /// The log file path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one feedback item to the log as a JSON line.
    ///
    /// # Errors
    /// Propagates serialization and I/O errors.
    pub fn append(&self, item: &FeedbackItem) -> io::Result<()> {
        self.write_line(&LogEntry::Item(item.clone()))
    }

    /// Append the end-of-review marker.
    ///
    /// # Errors
    /// Propagates I/O errors from writing the marker line.
    pub fn finalize(&self) -> io::Result<()> {
        self.write_line(&LogEntry::End(EndMarker { ended: true }))
    }

    /// Append a reopen marker (`{"ended":false}`) so a finished review accepts a new
    /// pass while keeping the log — and every reply in it — intact. The latest marker
    /// wins in [`Self::settled`], so this clears the ended state without discarding
    /// feedback, unlike [`Self::reset`], which wipes the log. Presenting an artifact is
    /// a request to review it, so the CLI reopens on a non-`--fresh` present; a
    /// never-finished review is already open, so callers gate this on the ended state
    /// to avoid marker noise.
    ///
    /// # Errors
    /// Propagates I/O errors from writing the marker line.
    pub fn reopen(&self) -> io::Result<()> {
        self.write_line(&LogEntry::End(EndMarker { ended: false }))
    }

    /// Record that a server (this `pid`) began serving the artifact at `port`/`url`. The
    /// log is the instance registry, so no side-car pidfile is needed.
    ///
    /// # Errors
    /// Propagates I/O errors from writing the event.
    pub fn record_serve(&self, pid: u32, port: u16, url: &str) -> io::Result<()> {
        self.write_line(&LogEntry::Life(Lifecycle::Serve {
            pid,
            port,
            url: url.to_string(),
        }))
    }

    /// Record that the server (this `pid`) stopped; `completed` is whether the review had
    /// been finished at that point.
    ///
    /// # Errors
    /// Propagates I/O errors from writing the event.
    pub fn record_shutdown(&self, pid: u32, completed: bool) -> io::Result<()> {
        self.write_line(&LogEntry::Life(Lifecycle::Shutdown { pid, completed }))
    }

    /// Discard the log so the review starts from scratch (the `--fresh` path). A
    /// missing log is already fresh, so that is not an error.
    ///
    /// # Errors
    /// Propagates I/O errors other than not-found.
    pub fn reset(&self) -> io::Result<()> {
        match fs::remove_file(&self.path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            other => other,
        }
    }

    /// The settled review state: for each anchor the latest item (a `delete` drops
    /// it), in first-seen order, plus whether the review has ended. A corrupt line
    /// is skipped, not fatal; a missing log is an empty state.
    ///
    /// # Errors
    /// Propagates I/O errors from reading the log (other than not-found).
    pub fn settled(&self) -> io::Result<(Vec<FeedbackItem>, bool)> {
        let r = self.replay()?;
        Ok((r.items, r.ended))
    }

    /// The review outcome for the poll: `(items, ended, closed)`. `closed` is true when
    /// the review was not finished **and** its serving instance is gone — either it
    /// recorded a shutdown for its pid, or that pid is no longer alive (a crash, kill, or
    /// a close beacon that never fired). `is_alive` is injected so the pid-liveness branch
    /// is testable without a real process, and so a crashed server never leaves the poll
    /// blocking forever.
    ///
    /// # Errors
    /// Propagates I/O errors from reading the log (other than not-found).
    pub fn outcome(
        &self,
        is_alive: impl Fn(u32) -> bool,
    ) -> io::Result<(Vec<FeedbackItem>, bool, bool)> {
        let r = self.replay()?;
        let closed = !r.ended
            && r.last_serve_pid
                .is_some_and(|pid| r.shutdown_pids.contains(&pid) || !is_alive(pid));
        Ok((r.items, r.ended, closed))
    }

    /// Fold the log once into settled items, the ended flag, and the serve/shutdown pids.
    /// A corrupt line is skipped, not fatal; a missing log is an empty replay.
    fn replay(&self) -> io::Result<Replay> {
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Replay::default()),
            Err(e) => return Err(e),
        };
        let mut r = Replay::default();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            match serde_json::from_str::<LogEntry>(line) {
                Ok(LogEntry::End(marker)) => r.ended = marker.ended,
                Ok(LogEntry::Item(item)) => merge(&mut r.items, item),
                Ok(LogEntry::Life(Lifecycle::Serve { pid, .. })) => r.last_serve_pid = Some(pid),
                Ok(LogEntry::Life(Lifecycle::Shutdown { pid, .. })) => r.shutdown_pids.push(pid),
                Err(_) => {} // corrupt line: skip, not fatal
            }
        }
        Ok(r)
    }

    /// Append one serialized log entry as a line, creating the file if needed.
    fn write_line(&self, entry: &LogEntry) -> io::Result<()> {
        let mut line = serde_json::to_string(entry).map_err(io::Error::other)?;
        line.push('\n');
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(line.as_bytes())
    }
}

/// Fold one item into the settled set by its `(kind, anchor)` identity: `delete`
/// removes the matching item, an existing `(kind, anchor)` is replaced in place
/// (preserving first-seen order), and otherwise only a `new` is appended — an
/// `update`/`delete` with no existing slot (e.g. replaying after a `delete`) is a
/// no-op, so a deleted thread is never resurrected by a later edit or resolve/reopen
/// (both of which post `update`s). Keying on `kind` as well as the anchor keeps an
/// answer, a block-level annotation, and a Gate-2 finding on the same block
/// independent — they share an anchor but are distinct feedback, so one must not
/// clobber another.
fn merge(items: &mut Vec<FeedbackItem>, item: FeedbackItem) {
    let (kind, anchor) = (item.kind, item.anchor());
    if item.status == FeedbackStatus::Delete {
        items.retain(|existing| existing.kind != kind || existing.anchor() != anchor);
    } else if let Some(slot) = items
        .iter_mut()
        .find(|existing| existing.kind == kind && existing.anchor() == anchor)
    {
        *slot = item;
    } else if item.status == FeedbackStatus::New {
        items.push(item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FeedbackKind, SubTarget};

    fn log(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("ags-log-test").join(name);
        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        dir.join("artifact.md.ags.jsonl")
    }

    fn item(
        block: &str,
        sub: Option<SubTarget>,
        status: FeedbackStatus,
        body: &str,
    ) -> FeedbackItem {
        let mut it = FeedbackItem::new(block, sub, FeedbackKind::Annotation, body).unwrap();
        it.status = status;
        it
    }

    #[test]
    fn missing_log_is_an_empty_state() {
        let s = Session::open_at(log("missing")).unwrap();
        assert_eq!(s.settled().unwrap(), (Vec::new(), false));
    }

    #[test]
    fn appends_are_settled_in_first_seen_order() {
        let s = Session::open_at(log("order")).unwrap();
        s.append(&item("a", None, FeedbackStatus::New, "first"))
            .unwrap();
        s.append(&item("b", None, FeedbackStatus::New, "second"))
            .unwrap();
        let (items, ended) = s.settled().unwrap();
        assert!(!ended);
        assert_eq!(
            items.iter().map(|i| i.body.as_str()).collect::<Vec<_>>(),
            ["first", "second"]
        );
    }

    #[test]
    fn update_overrides_in_place_and_delete_drops() {
        let s = Session::open_at(log("lifecycle")).unwrap();
        let node = || Some(SubTarget::Node("Auth".into()));
        s.append(&item("flow", node(), FeedbackStatus::New, "v1"))
            .unwrap();
        s.append(&item("flow", None, FeedbackStatus::New, "block note"))
            .unwrap();
        s.append(&item("flow", node(), FeedbackStatus::Update, "v2"))
            .unwrap();
        let (items, _) = s.settled().unwrap();
        // The Auth anchor keeps its slot (order) but shows the updated body.
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].anchor(), "#flow/node:Auth");
        assert_eq!(items[0].body, "v2");
        assert_eq!(items[1].body, "block note");
        // A delete on that anchor removes it entirely.
        s.append(&item("flow", node(), FeedbackStatus::Delete, "v2"))
            .unwrap();
        let (items, _) = s.settled().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].body, "block note");
    }

    #[test]
    fn feedback_of_different_kinds_on_one_block_coexist() {
        // An answer, a block-level annotation, and a finding on the same block share
        // the anchor `#q1` but are distinct kinds, so none clobbers another.
        let s = Session::open_at(log("kinds")).unwrap();
        for (kind, body) in [
            (FeedbackKind::Answer, "SQLite"),
            (FeedbackKind::Annotation, "reconsider this"),
            (FeedbackKind::Finding, "overflows its column"),
        ] {
            s.append(&FeedbackItem::new("q1", None, kind, body).unwrap())
                .unwrap();
        }
        let (items, _) = s.settled().unwrap();
        assert_eq!(
            items.len(),
            3,
            "distinct kinds on one anchor must all survive"
        );
        // A delete removes only the matching-kind item, leaving the answer and finding.
        let mut del = FeedbackItem::new("q1", None, FeedbackKind::Annotation, "x").unwrap();
        del.status = FeedbackStatus::Delete;
        s.append(&del).unwrap();
        let (items, _) = s.settled().unwrap();
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.kind != FeedbackKind::Annotation));
    }

    #[test]
    fn resolve_rides_through_settled_as_an_update() {
        let s = Session::open_at(log("resolve")).unwrap();
        s.append(&item("flow", None, FeedbackStatus::New, "reconsider"))
            .unwrap();
        // The reviewer resolves the thread: same anchor, an update carrying resolved.
        let mut done = item("flow", None, FeedbackStatus::Update, "reconsider");
        done.resolved = true;
        s.append(&done).unwrap();
        let (items, _) = s.settled().unwrap();
        assert_eq!(
            items.len(),
            1,
            "resolving replaces in place, never adds a row"
        );
        assert!(
            items[0].resolved,
            "the settled item carries the resolved flag"
        );
        // Reopening clears it — still one item, and the log kept every step (history).
        s.append(&item("flow", None, FeedbackStatus::Update, "reconsider"))
            .unwrap();
        let (items, _) = s.settled().unwrap();
        assert_eq!(items.len(), 1);
        assert!(!items[0].resolved, "reopen clears resolved");
    }

    #[test]
    fn an_update_after_a_delete_does_not_resurrect_the_item() {
        // Resolve/reopen post `update`s; if one replays after a delete for the same
        // anchor it must NOT re-add the deleted thread (only a `new` may create a slot).
        let s = Session::open_at(log("no-resurrect")).unwrap();
        s.append(&item("flow", None, FeedbackStatus::New, "x"))
            .unwrap();
        s.append(&item("flow", None, FeedbackStatus::Delete, "x"))
            .unwrap();
        let mut resolve = item("flow", None, FeedbackStatus::Update, "x");
        resolve.resolved = true;
        s.append(&resolve).unwrap();
        assert!(
            s.settled().unwrap().0.is_empty(),
            "an update after a delete must not resurrect the item"
        );
    }

    #[test]
    fn finalize_sets_ended_and_keeps_items() {
        let s = Session::open_at(log("ended")).unwrap();
        s.append(&item("q", None, FeedbackStatus::New, "SQLite"))
            .unwrap();
        assert!(!s.settled().unwrap().1);
        s.finalize().unwrap();
        let (items, ended) = s.settled().unwrap();
        assert!(ended);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn reopen_clears_ended_and_keeps_feedback() {
        // A finished review that is presented again reopens for a new pass, and the
        // prior feedback survives as history (unlike `reset`, which wipes it).
        let s = Session::open_at(log("reopen")).unwrap();
        s.append(&item("q", None, FeedbackStatus::New, "SQLite"))
            .unwrap();
        s.finalize().unwrap();
        assert!(s.settled().unwrap().1, "finished review is ended");
        s.reopen().unwrap();
        let (items, ended) = s.settled().unwrap();
        assert!(!ended, "reopen clears the ended state");
        assert_eq!(items.len(), 1, "feedback is kept across the reopen");
    }

    #[test]
    fn the_latest_end_marker_wins() {
        // finish -> reopen -> finish: the last marker decides, so the review is ended.
        let s = Session::open_at(log("last-marker")).unwrap();
        s.finalize().unwrap();
        s.reopen().unwrap();
        s.finalize().unwrap();
        assert!(s.settled().unwrap().1, "the last end-marker wins");
    }

    #[test]
    fn outcome_reports_closed_on_a_shutdown_paired_to_the_last_serve() {
        // A shutdown for the serving pid, with no finish, is an abandoned review.
        let s = Session::open_at(log("closed-graceful")).unwrap();
        s.record_serve(1234, 8770, "http://127.0.0.1:8770").unwrap();
        let (_, ended, closed) = s.outcome(|_| true).unwrap();
        assert!(!ended && !closed, "serving + alive = open");
        s.record_shutdown(1234, false).unwrap();
        let (_, ended, closed) = s.outcome(|_| true).unwrap();
        assert!(!ended && closed, "shutdown without finish = closed");
    }

    #[test]
    fn outcome_reports_closed_when_the_serving_pid_is_dead() {
        // Crash path: no shutdown event, but the recorded pid is gone.
        let s = Session::open_at(log("closed-dead-pid")).unwrap();
        s.record_serve(4242, 8770, "u").unwrap();
        assert!(!s.outcome(|_| true).unwrap().2, "alive pid = open");
        assert!(
            s.outcome(|_| false).unwrap().2,
            "dead pid with no shutdown = closed"
        );
    }

    #[test]
    fn outcome_finished_beats_closed_and_a_later_serve_reopens() {
        let s = Session::open_at(log("outcome-precedence")).unwrap();
        s.record_serve(1, 1, "u").unwrap();
        s.finalize().unwrap();
        let (_, ended, closed) = s.outcome(|_| false).unwrap();
        assert!(ended && !closed, "finished wins even with a dead pid");
        // Re-present: a later serve (fresh pid) after a shutdown returns to open.
        s.record_shutdown(1, true).unwrap();
        s.reopen().unwrap();
        s.record_serve(2, 1, "u").unwrap();
        let (_, ended, closed) = s.outcome(|_| true).unwrap();
        assert!(!ended && !closed, "a new serve re-opens the review");
    }

    #[test]
    fn corrupt_line_is_skipped_not_fatal() {
        let path = log("corrupt");
        let s = Session::open_at(&path).unwrap();
        s.append(&item("a", None, FeedbackStatus::New, "good"))
            .unwrap();
        // splice a garbage line into the log
        let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"not json\n").unwrap();
        s.append(&item("b", None, FeedbackStatus::New, "also good"))
            .unwrap();
        let (items, _) = s.settled().unwrap();
        assert_eq!(
            items.len(),
            2,
            "the corrupt line is dropped, the rest survive"
        );
    }

    #[test]
    fn settled_is_idempotent() {
        let s = Session::open_at(log("idempotent")).unwrap();
        s.append(&item("a", None, FeedbackStatus::New, "x"))
            .unwrap();
        assert_eq!(s.settled().unwrap(), s.settled().unwrap());
    }

    #[test]
    fn path_is_reported() {
        let p = log("path");
        let s = Session::open_at(&p).unwrap();
        assert_eq!(s.path(), p.as_path());
    }

    #[test]
    fn open_at_tolerates_a_bare_filename() {
        // A bare name has an empty parent, so open_at must skip directory creation.
        let s = Session::open_at(PathBuf::from("nonexistent-artifact.md.ags.jsonl")).unwrap();
        assert_eq!(s.settled().unwrap(), (Vec::new(), false));
    }

    #[test]
    fn reset_discards_the_log_and_is_idempotent() {
        let s = Session::open_at(log("reset")).unwrap();
        s.append(&item("a", None, FeedbackStatus::New, "x"))
            .unwrap();
        assert_eq!(s.settled().unwrap().0.len(), 1);
        s.reset().unwrap();
        assert_eq!(s.settled().unwrap(), (Vec::new(), false));
        s.reset().unwrap(); // already gone — a missing log is already fresh
    }
}
