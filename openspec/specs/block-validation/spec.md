# block-validation Specification

## Purpose
Gate 1: refusing an artifact that breaks the format before anyone is asked to read
it. Everything here is a rule that can be settled from the text plus the geometry
the renderer computes, with no browser and no network.
## Requirements
### Requirement: Near-miss fence types are rejected
Gate 1 SHALL reject a fenced block whose first info-string token is not a known block
type but is **one edit** away from one, and the error SHALL name both the token as
written and the block type it most likely intended.

This exists because an unrecognized fence type is now valid prose rather than a
validation failure, so a **mistyped** block type would otherwise degrade silently — a
diagram would render as a grey code block, and no gate would object.

"One edit" SHALL mean one insertion, one deletion, one substitution, **or one
transposition of adjacent characters** — Damerau-Levenshtein distance 1, not plain
Levenshtein. The transposition case is required, not a refinement: `mermiad` is two
plain-Levenshtein substitutions away from `mermaid`, so a plain edit-distance rule
would miss the single most likely typo this requirement exists to catch.

The threshold SHALL be exactly 1, which leaves every plausible language tag (`c`,
`go`, `rs`, `sh`, `js`, `ts`, `sql`, `toml`, `json`, `yaml`, `bash`, `diff`, `text`)
two or more edits from every member of the closed set, so legitimate code fences are
unaffected.

#### Scenario: A transposed block type is caught
- **WHEN** a fenced block opens with ` ```mermiad `
- **THEN** Gate 1 fails with an error naming `mermiad` and suggesting `mermaid`, and the artifact is not served

#### Scenario: A dropped character is caught
- **WHEN** a fenced block opens with ` ```nte ` or ` ```tabel `
- **THEN** Gate 1 fails with a did-you-mean error naming the intended type

#### Scenario: A legitimate language tag passes
- **WHEN** a fenced block opens with ` ```rust `, ` ```json `, or ` ```bash `
- **THEN** Gate 1 passes, because no such token is within edit distance 1 of a block type

#### Scenario: An exact block type is unaffected
- **WHEN** a fenced block opens with ` ```note `
- **THEN** it is parsed as a note block and near-miss detection does not apply

### Requirement: The CLI validates blocks before review
The CLI SHALL validate an agent-authored artifact before opening it for human
review, and SHALL return structured errors to the agent when validation fails, so
that a structurally broken artifact is never shown to a human. Validation SHALL be
deterministic and SHALL NOT require a browser. On success the CLI SHALL proceed to
serve the artifact; on failure it SHALL NOT open the browser.

#### Scenario: Invalid artifact is reported, not shown
- **WHEN** the agent presents an artifact whose validation fails
- **THEN** the CLI returns the errors to the agent and does not open the browser

#### Scenario: Valid artifact proceeds to review
- **WHEN** the agent presents an artifact that passes validation
- **THEN** the CLI serves the artifact and opens it for review

### Requirement: Gate 1 is engine-free and delegates its rules
Gate 1 SHALL enforce, without rendering and without invoking the diagram engine, the
structural and per-type rules defined by the `block-format` capability (block/fence
structure, info-string grammar, the closed block-type set, per-type schema, unique
element ids, and HTML-chunk safety) and the themed-content rules defined by the
`visual-system` capability. Gate 1 owns the gate mechanics — the enumerated rules are
owned by those capabilities and are not restated here. Diagram parse-validity SHALL
NOT be a Gate-1 check: the rendering engine is the single source of truth for it and
runs at the browser gate.

#### Scenario: Gate 1 rejects a rule violation before serving
- **WHEN** an artifact violates any `block-format` or `visual-system` rule (e.g. an unsafe HTML chunk, a non-rectangular table, or a duplicate `#id`)
- **THEN** Gate 1 fails with a structured error naming the offending block, and the artifact is not served

#### Scenario: Diagram validity is deferred, not checked at Gate 1
- **WHEN** a diagram block passes its `block-format` structural checks but its source is unrenderable
- **THEN** Gate 1 passes it and the failure surfaces at the browser gate, because the engine is the source of truth

### Requirement: Structured error output
When validation fails, the CLI SHALL emit errors as structured TOON on stdout (not
free prose, not stderr), each error identifying the offending block id, the kind of
failure, and an actionable detail, so the agent can fix and re-present
programmatically.

#### Scenario: Errors are structured and addressable
- **WHEN** validation fails for two blocks
- **THEN** stdout contains a TOON error collection with one row per failure, each carrying the block id, failure kind, and detail

### Requirement: In-browser render gate (required)
The tool SHALL run an in-browser render audit on every served artifact and SHALL
treat it as a required second gate, because a CLI parser cannot determine how
agent-authored HTML chunks or dense diagrams actually lay out, and agents reliably
produce HTML that renders badly. After fonts and layout settle, the audit SHALL
detect render failures that require real geometry — page or element overflow,
clipped text, overlapping text — across all blocks including HTML chunks, and SHALL
also surface any diagram that fails to render (e.g. unparseable diagram source).
When the audit finds error-severity problems, the tool SHALL hold the artifact
behind a curtain so the human is not shown a broken render, and SHALL report the
findings to the agent through the `feedback-transport` channel. The curtain SHALL lift on a
clean re-render, with a bounded "show anyway" safety valve so review is never
blocked indefinitely.

#### Scenario: Bad HTML chunk is held at the gate
- **WHEN** an artifact passes CLI validation but an HTML chunk renders with element overflow or clipped text
- **THEN** the render audit holds the artifact behind the curtain and reports the finding to the agent
- **AND** the human is not shown the broken render until a clean re-render

#### Scenario: Unrenderable diagram surfaces at the browser gate
- **WHEN** a diagram block's source passes CLI structural checks but the engine cannot render it
- **THEN** the render failure surfaces at the browser gate and is reported to the agent, before human review

#### Scenario: Clean artifact reveals normally
- **WHEN** the render audit finds no error-severity problems
- **THEN** the artifact is revealed to the human without a curtain

### Requirement: The served page is the HTML-safety boundary, not the CLI
The tool SHALL serve the artifact page with a Content-Security-Policy that blocks
script execution, inline event handlers, and `javascript:`/`data:` navigation, so
that the **browser** is the enforcing boundary for agent-authored HTML safety. The
Gate-1 HTML-chunk check (whitelisted tags, no `<script>`/handlers, no unsafe URL
scheme) is a deterministic **fast-fail hint** that gives the agent early, structured
feedback; it SHALL NOT be relied on as the security boundary, and in particular SHALL
NOT be required to catch obfuscated (e.g. entity-encoded) scheme evasions, which the
CSP neutralizes uniformly after the browser decodes them. This mirrors the
engine-is-source-of-truth split used for diagram validity: the CLI does the cheap,
decidable check; the browser does the authoritative one.

#### Scenario: Obfuscated scheme slips the fast-fail but is neutralized by the CSP
- **WHEN** an HTML chunk carries an entity-encoded `javascript:` URL that the Gate-1 fast-fail does not decode
- **THEN** the CLI may pass it, and the served page's CSP prevents it from executing in the reviewer's browser

