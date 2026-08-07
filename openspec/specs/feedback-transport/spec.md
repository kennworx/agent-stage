# feedback-transport Specification

## Purpose
Carrying a reviewer's answers back to the agent that asked. Feedback is anchored
to the block it responds to, routed by whether a human or an agent is answering,
and delivered by polling, so the host needs no callback address.
## Requirements
### Requirement: Anchored feedback
Every feedback item SHALL name the block it targets by `#id`, and MAY carry a
sub-target identifying an element within that block: a diagram node by its `data-id`,
a table cell by row and column, a code line by 1-based line or range, or a prose text
range by quoted text with surrounding context. An answer to a `question` block SHALL
target that block's `#id`. Feedback that names no resolvable target SHALL be rejected.

#### Scenario: Diagram-node annotation is anchored by id
- **WHEN** a human annotates the `Auth` node in a `mermaid #flow` block
- **THEN** the feedback item carries block `#flow` and sub-target node `Auth` (its `data-id`)

#### Scenario: Code-line annotation carries a line range
- **WHEN** a human annotates lines 12–18 of a `code #snippet` block
- **THEN** the item carries block `#snippet` and sub-target line range `12-18`

### Requirement: Agent-vs-human routing
Every feedback item SHALL carry a `resolutionTarget` of `agent` or `human`. Only
items with `resolutionTarget=agent` SHALL be treated as an actionable routing signal
for the agent; `human`-targeted items SHALL be delivered as context only, and
mentions SHALL be treated as notifications, never as routing.

#### Scenario: Agent acts only on agent-targeted items
- **WHEN** a poll returns one `agent`-targeted comment and one `human`-targeted comment
- **THEN** the agent is directed to act on the `agent` item and to treat the `human` item as context

### Requirement: Poll delivery
The agent SHALL receive feedback by long-polling. The poll SHALL return queued human
annotations, human answers, and browser render-audit findings as structured TOON, and
SHALL block silently until the human sends feedback, fresh render findings arrive, the
human finishes the review, or the review is **closed** (the serving process stopped without
a finish). The poll output SHALL carry both an `ended` flag (the review was finished) and a
`closed` flag (the reviewer left without finishing), so the agent distinguishes a completed
review from an abandoned one; `ended` remains for backward compatibility rather than being
replaced. The poll SHALL treat the serving process as gone — and report `closed` — when the
log records a shutdown for the serving instance **or** the serving instance's process is no
longer alive, so a crash or a beacon that never fired does not leave the poll blocking
forever. The poll SHALL mark the items it returns as **delivered** so a subsequent poll does
not return them again, and an interrupted poll SHALL be safe to re-run without losing queued
feedback.

#### Scenario: Poll blocks then returns queued feedback
- **WHEN** the agent polls an artifact with no pending feedback and the human then sends an annotation
- **THEN** the poll returns the annotation as structured output

#### Scenario: Delivered items are not returned twice
- **WHEN** the agent polls, receives an annotation, and later polls again with no new feedback
- **THEN** the already-delivered annotation is not returned a second time

#### Scenario: Render findings wake the poll
- **WHEN** the browser render audit reports an error-severity finding while the agent is polling
- **THEN** the poll returns that finding through the same channel

#### Scenario: A closed-without-finish review wakes the poll as not-completed
- **WHEN** the human closes the review tab without finishing and the server shuts down
- **THEN** the poll stops blocking and reports `closed` (and not `ended`), so the agent learns the wait ended but the review was not completed

#### Scenario: A crashed server does not block the poll forever
- **WHEN** the serving process dies without recording a shutdown (crash/kill) and its pid is no longer alive
- **THEN** the poll reports `closed` on the strength of the dead recorded pid, rather than blocking

### Requirement: Finalize ends the loop
A human "finish review" action SHALL end the review; the next poll SHALL report `ended` so
the agent stops polling, and feedback queued together with the finish SHALL be delivered
before that. On a successful finish the viewer SHALL become read-only — offering no way to
add, edit, resolve, or delete review items — and SHALL prompt the human to close the tab; it
SHALL NOT depend on `window.close()`, which cannot close an OS-opened tab. Closing the tab
after a finish SHALL shut the server down as a **completed** close; closing before a finish
SHALL shut it down as a **closed/abandoned** review, reported distinctly per "Poll delivery".

#### Scenario: Finish returns ended and the page goes fully read-only
- **WHEN** the human clicks finish review
- **THEN** the poll returns any queued feedback then reports `ended`, and the viewer becomes read-only with no mutation controls, showing a "you can close this tab" prompt

#### Scenario: Closing without finishing is an abandoned review
- **WHEN** the human closes the tab before finishing
- **THEN** the server shuts down and records the review as not completed, and the agent's poll reports `closed`

### Requirement: Two-axis state and history
Beyond the v1 **delivered** marker (Poll delivery), the tool SHALL track a
reviewer-facing **resolved** state independently of the agent-facing
**delivered/consumed** state, and SHALL NOT delete an item when it is delivered or
resolved, so the full review history is preserved. This makes an item that was
delivered-but-not-yet-resolved a first-class, persistent state.

#### Scenario: Resolved is independent of delivered, and history is kept
- **WHEN** the agent has been delivered an annotation but the reviewer has not marked it resolved
- **THEN** the item is tracked as delivered-and-unresolved, is not re-delivered, and remains in the review history until resolved

### Requirement: Detached-anchor reconciliation
The tool SHALL surface a feedback item as **detached** — rather than dropping it —
whenever the artifact re-renders and the item's anchor no longer resolves against the
new content (rewritten prose whose quoted text is gone, or a removed diagram node id),
so the agent can reconcile it against the new content.

#### Scenario: A rewritten block detaches its annotation
- **WHEN** the agent rewrites a block so an annotation's anchor no longer matches
- **THEN** that annotation is returned as detached and is not silently discarded

### Requirement: Local-first, path-keyed store
Feedback SHALL be stored locally and keyed by the canonical (realpath-resolved)
artifact file path, with no database and no hosted service. The artifact path SHALL
serve as the session identity so the tool does not require opaque session ids.

#### Scenario: Two paths to the same file share one session
- **WHEN** the artifact is addressed through a symlink and through its real path
- **THEN** both resolve to the same feedback session

### Requirement: Server lifecycle recorded in the review log
The review log SHALL record the serving instance and its shutdown as events, so no side-car
pidfile is needed. On start the server SHALL append a `serve` event carrying at least its
process id and port; on graceful stop it SHALL append a `shutdown` event carrying the same
pid and whether the review was completed. Pairing shutdown to serve by pid SHALL keep the
status correct when more than one instance serves the artifact. The viewer SHALL notify the
server of a tab close via a beacon to a shutdown route; the server SHALL debounce that
shutdown with a short grace window and cancel it if the page reconnects (a full page load),
so a reload does not stop the server, while background polls and the beacon itself do not
cancel. A shutdown-route hit that is not cancelled SHALL break the serve loop so the process
exits.

#### Scenario: The live instance is discoverable from the log
- **WHEN** a server starts serving an artifact
- **THEN** a `serve` event with its pid and port is appended, so a launcher can find and signal the process without a pidfile

#### Scenario: A reload does not shut the server down
- **WHEN** the review page reloads (firing the close beacon) and re-requests the page within the grace window
- **THEN** the pending shutdown is cancelled and the server keeps serving

#### Scenario: A shutdown is recorded and paired to its instance
- **WHEN** the server stops (tab closed or shutdown requested)
- **THEN** a `shutdown` event carrying its pid and whether the review completed is appended, and the process exits, without closing a different instance still serving the artifact

