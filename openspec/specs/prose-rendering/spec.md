# prose-rendering Specification

## Purpose
Turning the markdown around the blocks into the page's prose, in Rust. GFM, with
raw HTML escaped rather than passed through: an artifact is agent-authored, so
anything that would execute is treated as text.
## Requirements
### Requirement: Prose renders to HTML in Rust

Prose, tables, code blocks and box art SHALL be rendered to HTML by the Rust
renderer, preserving the behaviour the viewer's markdown pipeline provides today.

#### Scenario: GitHub Flavored Markdown renders server-side

- **WHEN** an artifact containing prose is served or baked
- **THEN** the page carries the finished HTML for headings, lists, tables,
  emphasis, inline code and fenced code
- **AND** no markdown parsing happens in the browser

#### Scenario: A wide table stays inside its column

- **WHEN** prose contains a table wider than the content column
- **THEN** it is wrapped in the same scroll container the current renderer uses

### Requirement: Raw HTML in prose is escaped, not passed through

Prose rendering SHALL escape embedded HTML rather than emit it.

The current pipeline is safe by configuration rather than by sanitising after the
fact, and moving to a server-side renderer must not quietly turn a rendering
change into an injection surface — the output is now assembled by the same
process that serves it.

#### Scenario: Markup in prose is displayed, not executed

- **WHEN** prose contains a raw HTML tag or an event-handler attribute
- **THEN** the page displays it as text
- **AND** creates no corresponding element

### Requirement: Link and anchor behaviour is preserved

Heading anchors, wiki links and unreachable-link hints SHALL behave as they do
today.

These were specified and built deliberately; a port that silently drops them
would be a regression disguised as an implementation change.

#### Scenario: Headings get stable anchors

- **WHEN** prose contains headings
- **THEN** each receives the same slug the current renderer produces
- **AND** the table of contents links resolve to them

#### Scenario: An unreachable link renders as a hint

- **WHEN** prose contains a wiki link, or a link whose target is not reachable
- **THEN** it renders as text carrying its target as a hint, not as a navigable link

