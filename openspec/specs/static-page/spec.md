# static-page Specification

## Purpose
Baking a whole artifact into one standalone HTML file — finished markup that opens
from disk with no server, no network and nothing to fetch. What script a served
page does carry is bounded and named by the CSP.
## Requirements
### Requirement: Fully rendered markup

The served and baked document SHALL contain finished HTML and inline SVG for
every block whose type the Rust renderer supports. The client MUST NOT be
required to run any script in order to read that content.

A reader on a phone should see the artifact on first paint. Deferring rendering
to the client is what makes that impossible today, and no amount of caching fixes
it while every `present` run is a different origin.

#### Scenario: A document renders with scripting disabled

- **WHEN** a served or baked page whose diagrams are all Rust-rendered is opened with JavaScript disabled
- **THEN** every prose block, table, code block and diagram is fully visible
- **AND** no element is empty, placeholder, or awaiting hydration

#### Scenario: No request leaves the page

- **WHEN** such a baked page is opened offline
- **THEN** the document issues no network request for fonts, scripts, styles or images
- **AND** every asset it needs is inline

### Requirement: Bounded client script

Any script the page carries SHALL be limited to
recording reviewer input — comments and question answers. It MUST NOT perform
layout, markdown rendering, or theming.

Keeping the boundary explicit is what stops the client bundle growing back: every
addition has to justify itself against a stated limit rather than against
whatever is already there.

#### Scenario: A baked page carries no behaviour of its own

- **WHEN** a page is produced by `ags bake` and uses only Rust-rendered diagrams
- **THEN** it contains no script, because a baked page is read-only

#### Scenario: A served page carries only the feedback surface

- **WHEN** a page is produced by `ags present`
- **THEN** its own script posts comments and answers, and does nothing else

### Requirement: Theming works without script

The active theme SHALL be applied by emitted CSS, and switching between light and
dark SHALL work on a page carrying no script.

Baked pages have a working theme toggle today. Moving rendering to the server
must not quietly remove it, and a page that renders without script should be
themeable without script.

#### Scenario: A scriptless page is correctly themed

- **WHEN** a baked page is opened with JavaScript disabled
- **THEN** it renders in its theme, because the token values were emitted with the document

#### Scenario: The toggle works without script

- **WHEN** the reader activates the theme control on a page carrying no script
- **THEN** the document restyles

### Requirement: Reviewer feedback survives the change

A served page SHALL continue to accept the feedback a reviewer can give today:
comments on a block, answers to a question block, and comments keyed to an
individual diagram element.

The feedback loop is the reason `present` exists. Replacing the viewer that
implements it is the point at which it could be lost by omission rather than by
decision.

#### Scenario: A block comment is recorded

- **WHEN** a reviewer comments on a block of a served artifact
- **THEN** the comment is recorded against that block and survives a reload

#### Scenario: An element-level comment is recorded

- **WHEN** a reviewer selects an element within a rendered diagram and comments
- **THEN** the comment is recorded against that element's identity

#### Scenario: A question is answered

- **WHEN** a reviewer answers a question block
- **THEN** the answer is recorded and reflected on reload

### Requirement: A broken anchor is reported by the host, not looked for in a page

The poll response SHALL carry, for each item, whether its anchor still resolves
against the artifact as it stands when the response is built. The item SHALL be
returned either way.

This reconciliation used to be a DOM walk in the viewer: render the page, then look
for the element the anchor names. It therefore required a page — an agent polling a
review the human had closed got no answer at all. The renderer runs in the host now
and knows every `data-id` it drew, so the same question is a set difference over
what the artifact currently offers, answerable with no browser open.

It SHALL be derived per response and SHALL NOT be stored on the item. Whether an
anchor resolves is a fact about the artifact, not about the comment: the same
recorded comment is attached before a redraw and detached after, so storing one
verdict would freeze it over every later render.

#### Scenario: A redrawn diagram detaches the comment on a node it dropped

- **WHEN** a reviewer has commented on a diagram element and the agent redraws the
  diagram without it
- **THEN** the poll returns that comment marked detached, rather than dropping it

#### Scenario: Restoring the element re-attaches its comment

- **WHEN** an element a detached comment names is drawn again
- **THEN** the comment is reported attached, with nothing to undo

#### Scenario: The artifact is answered against as it stands at reply time

- **WHEN** the artifact is rewritten while a poll is blocked waiting for feedback
- **THEN** anchors are resolved against the rewritten artifact, not the one that
  existed when the poll began

