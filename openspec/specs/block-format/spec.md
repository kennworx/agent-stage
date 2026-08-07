# block-format Specification

## Purpose
The closed vocabulary an agent authors against: which fenced block types exist,
what their info strings may say, and how a block is identified. Closed on purpose
— an open set cannot be validated before a reader sees it.
## Requirements
### Requirement: Artifact is markdown with fenced blocks
An artifact SHALL be a markdown document in which structured or interactive content
is expressed as fenced blocks whose info string names a block type, and in which raw
markdown between fenced blocks is rendered as implicit prose. Implicit prose SHALL NOT
require a fence or an id; fenced blocks SHALL be the unit that carries an id and a
schema.

Implicit prose SHALL be rendered as **GitHub Flavored Markdown**: headings at every
level, ordered and unordered lists, tables, blockquotes, thematic breaks, fenced code,
inline code, emphasis, strikethrough, images, and links. Raw HTML in prose SHALL be
escaped and rendered as literal text, never interpreted as markup, so the rendered tag
set is fixed by the renderer and cannot be widened by an author.

#### Scenario: Prose and blocks coexist
- **WHEN** an artifact contains a markdown paragraph followed by a ` ```mermaid ` fenced block
- **THEN** the paragraph renders as prose and the fenced block renders as a diagram
- **AND** the paragraph requires no id while the fenced block may carry one

#### Scenario: Prose renders GFM constructs as markup
- **WHEN** implicit prose contains a pipe table, a bullet list, a blockquote, and a heading
- **THEN** each renders as its corresponding markup, and none appears as literal source text

#### Scenario: Raw HTML in prose is inert
- **WHEN** implicit prose contains `<script>alert(1)</script>` or `<div onclick="…">`
- **THEN** the text is escaped and displayed literally, and no element or handler enters the document

### Requirement: Closed block-type set
The closed set SHALL be the set of **addressable** block types — those that carry an
id, a schema, and a review affordance — and SHALL be exactly: `mermaid`, `question`,
`table`, `code`, `html`, `note`, and `theme`.

A fenced block whose first info-string token is one of these SHALL be parsed as a
block and validated against that type's schema. A fenced block whose first
info-string token is anything else, **including an empty info string**, SHALL NOT be
a validation failure: it SHALL remain part of the surrounding implicit prose and be
rendered by the prose renderer as an ordinary fenced code block, carrying its info
string as a language hint and receiving no id, no schema, and no review affordance.

An unrecognized fence SHALL still be tracked to its closing delimiter, and its
opening and closing delimiter lines SHALL be preserved verbatim in the prose, so that
a block-type name appearing inside an unrecognized fence's body is not mistaken for
the start of a block.

The classification rule SHALL be identical in the validator and in the renderer.

#### Scenario: An unrecognized type is prose, not an error
- **WHEN** a fenced block opens with ` ```rust `
- **THEN** validation passes, and the fence renders as a fenced code block with `rust` as its language hint

#### Scenario: A fence with no info string is prose
- **WHEN** a fenced block opens with a bare ` ``` `
- **THEN** validation passes and the fence renders as a fenced code block with no language hint

#### Scenario: A block name inside an unrecognized fence does not open a block
- **WHEN** a ` ```text ` fence's body contains a line reading ` ```mermaid `
- **THEN** the whole span is one code block in the prose, and no diagram block is parsed

#### Scenario: A recognized type is still a block
- **WHEN** a fenced block opens with ` ```mermaid #d1 feedback=annotate `
- **THEN** it is parsed as a diagram block, validated against the mermaid schema, and carries its id and affordance

### Requirement: Info-string grammar and attributes
A block's info string SHALL follow `<type> [#id] [key=value | flag]*` with the type as
the first token. Every type SHALL accept the universal attributes `#id`,
`feedback` (`none|annotate|comment`), `title`, and `collapsible`. Each type SHALL
accept only its own declared type-specific attributes; an unrecognized attribute key
SHALL fail validation. When `feedback` is set to a value other than `none`, the block
SHALL carry an `#id`.

#### Scenario: Type-specific attribute is validated against the type
- **WHEN** a `question` block declares `type=radio` and a `mermaid` block declares `mode=live`
- **THEN** both validate, because each attribute is valid for its type
- **AND** a `mermaid` block declaring `type=radio` fails, because `type` is not a mermaid attribute

#### Scenario: Feedback requires an id
- **WHEN** a block declares `feedback=annotate` but no `#id`
- **THEN** validation fails, because feedback must route to an addressable element

### Requirement: Per-type body and validation
Each block type SHALL define its body format and Gate-1 validation rules:
`mermaid` body is diagram source (non-empty; parse-validity deferred to the browser
gate); `question` body is a prompt followed by a markdown list of options, requires a
`type`, and requires at least two options for `radio`/`checkbox`/`select`; `table`
body is a markdown table that SHALL be rectangular (every row's cell count equals the
header's); `code` body is source text and requires a known `lang`; `html` body SHALL
contain only whitelisted tags — which include a **curated static inline-SVG subset** —
with no `<script>` or event-handler attributes and no unsafe URL scheme. Non-SVG
themed content SHALL follow the themed-content rules (color via shadcn `var(--token)`
custom properties rather than hardcoded literals, no `font-family`, no absolute
positioning) defined by the `visual-system` capability; **inline SVG is image-class
content exempt from those rules** per that capability, so vector art may carry fixed
paint values. `note` body is markdown and SHALL be non-empty.

#### Scenario: Non-rectangular table is rejected
- **WHEN** a `table` block's header declares three columns but a row has two cells
- **THEN** validation fails naming the block id and the arity mismatch

#### Scenario: Radio question needs options
- **WHEN** a `question type=radio` block lists fewer than two options
- **THEN** validation fails naming the block id

#### Scenario: Mermaid body validity is deferred
- **WHEN** a `mermaid` block has a non-empty body that is later unrenderable
- **THEN** Gate 1 passes the block on body-non-empty, and the render failure surfaces at the browser gate

#### Scenario: Inline SVG art passes with fixed paint
- **WHEN** an `html` block contains an inline `<svg>` using curated shapes and `fill="#f59e0b"`
- **THEN** Gate 1 passes it, because SVG is image-class and its paint literals are not themed-content violations

### Requirement: Feedback affordance per type
Each block type SHALL expose a defined feedback affordance: `mermaid` SHALL support
per-node annotation (keyed by the node `data-id`) and live edit when `mode=live`;
`question` SHALL support a structured answer; `table` SHALL support per-cell
annotation; `code` SHALL support per-line annotation and whole-block comment; `html`
and `note` SHALL support annotation and comment. Annotations SHALL be keyed to the
block `#id` (and a sub-target for cells, lines, or nodes) and delivered to the agent
through the `feedback-transport` capability.

#### Scenario: Diagram node annotation is keyed by id and node
- **WHEN** a human annotates the `Auth` node in a `mermaid #flow` block
- **THEN** the feedback routes to the agent keyed to block `#flow` and node `Auth`

#### Scenario: Question yields a structured answer
- **WHEN** a human selects an option in a `question #commit type=radio` block
- **THEN** the feedback routes to the agent as an answer keyed to `#commit`

