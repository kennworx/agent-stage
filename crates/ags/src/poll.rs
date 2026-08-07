//! The `poll` command — the return leg.
//!
//! Opens the artifact's feedback session and long-polls it, printing delivered
//! items + the ended status as TOON to stdout (heartbeats go to stderr). Draining
//! moves items to `delivered/`, so an interrupted poll is safe to re-run.
//!
//! The store root and poll timing are read from the environment (with production
//! defaults) so the loop is a real long-poll in use but bounded and fast in tests.

use std::io;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

use ags_feedback::poll_blocking;

/// Default heartbeat interval and how many intervals a single poll blocks for.
const DEFAULT_TICK_MS: u64 = 1000;
const DEFAULT_MAX_TICKS: u32 = 600;

/// Run `poll <file>`: long-poll the session and print the TOON response.
pub fn run(path: &str) -> ExitCode {
    match poll_session(path) {
        Ok(toon) => {
            println!("{toon}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("ags: poll on {path} failed: {err}");
            ExitCode::from(2)
        }
    }
}

/// Open the session and long-poll it, returning the TOON response.
///
/// The artifact is read when the response is built, not here: a poll blocks for
/// minutes, and whether an anchor still resolves is a question about the file as
/// it is when the agent is answered. A file that has since been deleted reads as
/// empty, under which every anchor is reported detached — which is what happened.
fn poll_session(path: &str) -> io::Result<String> {
    let session = crate::store::open_session(Path::new(path))?;
    let interval =
        Duration::from_millis(env_u64("AGS_POLL_INTERVAL_MS").unwrap_or(DEFAULT_TICK_MS));
    let max_ticks = env_u32("AGS_POLL_MAX_TICKS").unwrap_or(DEFAULT_MAX_TICKS);
    // The one place that knows both halves: `ags-feedback` carries the items and
    // has no idea what a block id means; `ags-render` can say whether one still
    // resolves. Joining them is the binary's job, which is why neither crate
    // depends on the other for it.
    poll_blocking(&session, interval, max_ticks, heartbeat, || {
        let source = std::fs::read_to_string(path).unwrap_or_default();
        let anchors = ags_render::anchors(&source);
        move |item: &ags_feedback::FeedbackItem| anchors.detached(item)
    })
}

/// Emit a heartbeat to stderr — stdout carries only the response.
fn heartbeat() {
    eprint!(".");
}

fn env_u64(var: &str) -> Option<u64> {
    std::env::var(var).ok()?.parse().ok()
}

fn env_u32(var: &str) -> Option<u32> {
    std::env::var(var).ok()?.parse().ok()
}
