# artifact-authoring Specification

## Purpose
What an agent writes, and why it writes it that way. An artifact is one markdown
file of prose and blocks that stands as the reasoning being reviewed — so the
discipline here is about what belongs in it, not about how it is rendered.
## Requirements
### Requirement: Block catalog command
The tool SHALL provide a command that prints the live block vocabulary — every
supported block type and its schema (required and optional attributes, body format)
— derived from the closed block set. The catalog SHALL additionally list the
diagram types the renderer supports, generated from the renderer itself so the
two cannot drift. The authoring skill SHALL direct the agent to
read the catalog before authoring rather than relying on memorized tags.

#### Scenario: Catalog reflects the closed set
- **WHEN** the agent runs the block-catalog command
- **THEN** it receives every supported block type with its attributes and body format, and no type outside the closed set

#### Scenario: Catalog lists renderable diagram types
- **WHEN** the agent runs the block-catalog command
- **THEN** it receives the diagram types the renderer can draw, so it does not author a diagram Gate 1 will reject

#### Scenario: Agent authors from the catalog
- **WHEN** the agent is about to author an artifact
- **THEN** the skill directs it to consult the catalog first, so it does not emit a block or attribute the validator would reject

### Requirement: Authoring discipline
The authoring skill SHALL direct the agent to build an artifact only when a human
needs to see, compare, comment on, or approve a direction (and to skip trivial work,
never pad, and never ship a single-step artifact); to lead with a concrete example
before abstractions; to write the artifact so it stands alone without chat history
and without revision language; to preserve the user's level of abstraction; to state
settled decisions as settled and defer only genuinely-open choices; to collect open
choices in a single bottom "Open Questions" block with recommended defaults; and to
close with a verification that exercises the real workflow.

#### Scenario: A settled decision is stated, not offered as a menu
- **WHEN** the agent has already chosen an approach with rationale
- **THEN** it states the decision as settled and does not also present it as an open choice

#### Scenario: Open choices live in one place
- **WHEN** two decisions remain genuinely open and would change the design
- **THEN** both appear only in the single bottom Open Questions block, each with a recommended default, and are not duplicated earlier

#### Scenario: Trivial work is not padded into an artifact
- **WHEN** the requested change is a one-line, unambiguous fix
- **THEN** the skill directs the agent to make the change directly rather than produce a padded artifact

### Requirement: The artifact is the approval gate
The authoring skill SHALL treat presenting the artifact as the request for approval,
SHALL keep authoring read-only until the human approves, and SHALL treat the artifact
(not the chat) as the source of truth when scope changes.

#### Scenario: Presenting is the ask
- **WHEN** the agent surfaces a finished artifact for review
- **THEN** it asks for approval by that presentation and does not add a separate "does this look good?" question

#### Scenario: No source edits before approval
- **WHEN** the agent is authoring or the human is still reviewing
- **THEN** the agent makes no source changes until the human approves the direction

### Requirement: Adversarial self-review before handoff
For high-stakes artifacts, the authoring skill SHALL run one skeptical review pass
before treating the artifact as final. The pass SHALL be non-blocking (the artifact
is surfaced first and reviewed concurrently), SHALL look for what is weak, missing,
or wrong rather than confirm the artifact, SHALL fix clear-cut defects in place, and
SHALL route genuine judgment calls to the Open Questions block rather than deciding
them silently.

#### Scenario: Skeptic finds an unstated hard-to-reverse decision
- **WHEN** the self-review finds a decision that is expensive to undo and was made implicitly
- **THEN** the agent either commits to it with rationale or adds it to Open Questions, and does not leave it buried

#### Scenario: Review runs without blocking handoff
- **WHEN** a high-stakes artifact is ready
- **THEN** it is surfaced for the human immediately and the self-review runs in parallel, not before handoff

### Requirement: Visual discipline
The authoring skill SHALL direct the agent to add no visual chrome by default, to use
Mermaid for standard graph relationships and themed HTML for custom/spatial ones, to
prefer two-dimensional layouts over left-to-right chains where the relationship is not
sequential, and to use the sketch aesthetic to signal a provisional draft.

#### Scenario: Diagram engine matches the relationship
- **WHEN** the agent needs a spatial before/after or matrix
- **THEN** it authors a themed HTML diagram rather than forcing it into a Mermaid chain

### Requirement: Improve guidance, not one artifact
The authoring skill SHALL direct the agent, when a human critiques an artifact's look
or structure (as opposed to its content), to improve the renderer or the skill
guidance rather than hand-edit a single stored artifact.

#### Scenario: A look critique becomes better guidance
- **WHEN** a human says a class of artifact looks wrong structurally
- **THEN** the agent improves the shared renderer/skill guidance rather than patching only the current artifact

### Requirement: Fence only what needs an affordance
The authoring skill SHALL direct the agent to choose between a fenced block and plain
markdown on the basis of **review affordance**, not habit — a distinction that now
matters because an unrecognized fence is valid prose rather than a validation error.

The skill SHALL direct the agent to fence a block when the human needs to act on that
content — annotate a diagram node, comment on a table cell or a code line, answer a
question or a claim, or switch a theme — and to write plain markdown otherwise. It
SHALL state explicitly that a language-tagged fence such as ` ```rust ` is prose and
carries no line-annotation affordance, so an agent that wants a reviewer to comment on
a specific line must use the `code` block with its `lang` attribute instead.

The skill SHALL note that a prose table and a `table` block render identically, and
that the difference between them is solely whether a reviewer can annotate a cell.

#### Scenario: Content needing annotation is fenced
- **WHEN** the agent wants a reviewer to be able to comment on a specific line of an excerpt
- **THEN** it authors a `code` block with a `lang` attribute rather than a ` ```rust ` fence

#### Scenario: Ordinary illustrative content stays prose
- **WHEN** the agent includes a short snippet purely to illustrate a point, with nothing to review line-by-line
- **THEN** it writes an ordinary language-tagged fence in prose and adds no id

#### Scenario: The table choice is stated as an affordance choice
- **WHEN** the agent needs a data table and does not need per-cell comments
- **THEN** it writes a plain markdown table, and the skill does not treat that as a lesser form

