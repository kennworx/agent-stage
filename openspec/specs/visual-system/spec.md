# visual-system Specification

## Purpose
How a page looks, and why the renderer owns that rather than the artifact. A
semantic token kit gives themes somewhere safe to attach, so an agent can restyle
a page without being able to break it.
## Requirements
### Requirement: Renderer-owned look via a semantic token kit
The renderer SHALL own the visual look (footprint, font, icons, sizing, theme) and
the agent SHALL supply only semantic content. All visual color in themed content
SHALL reference semantic design tokens (CSS custom properties using shadcn/ui's
names: `--background`, `--foreground`, `--card`, `--muted-foreground`, `--border`,
`--primary`, `--primary-foreground`), and a helper-class kit and named icon
markers SHALL be available so common UI reads correctly without custom styling.
(`--radius` is renderer-owned sizing, not an agent-set color; `warn`/`ok` status
colors are a deferred additive extension.)

#### Scenario: Themed content reads correctly in both themes
- **WHEN** a themed block styles a container with `border: 1px solid var(--border)` and `background: var(--card)`
- **THEN** it renders correctly under both light and dark themes because the tokens flip, with no per-artifact patch

### Requirement: Themed content is safe by construction
Gate 1 (the engine-free CLI) SHALL reject, within themed content, any hardcoded
color literal in an inline `style` — hex, a CSS color function
(`rgb`/`hsl`/`hwb`/`lab`/`lch`/`oklab`/`oklch`/`color`), or a CSS named color —
allowing only the theme-neutral keywords `transparent`/`currentcolor`; any
`font-family` declaration; and absolute or fixed positioning. The color scan
SHALL be property-agnostic (every declaration's value, not only `color`/
`background`). This SHALL be a structural check requiring no rendering engine.
Host-framework color *classes* need no separate check: the renderer serves a
closed stylesheet with no framework CSS, so such a class is inert. Because raw
color/font/positioning cannot be authored, the browser render gate's role for
themed content narrows to true layout geometry. These restrictions SHALL apply to
themed content only; content inside an inline `<svg>` subtree is **image-class** (see
"Inline SVG is safe image-class content") and SHALL be exempt.

#### Scenario: Hardcoded color is rejected before serving
- **WHEN** a themed block contains `style="color:#fff;background:#111"`
- **THEN** Gate 1 fails with an error naming the block and the hardcoded-color rule, and the artifact is not served

#### Scenario: A hardcoded color in any property is rejected
- **WHEN** a themed block sets `style="border-color: red"` or `style="box-shadow: 0 0 4px #000"`
- **THEN** Gate 1 fails, because the color scan reads every declaration's value, not only `color`/`background`

#### Scenario: Semantic-token styling passes
- **WHEN** a themed block uses only `var(--token)` colors, helper classes, and inline flex/grid with literal lengths
- **THEN** Gate 1 passes the block

#### Scenario: The exemption is scoped to the SVG subtree
- **WHEN** an artifact has an inline `<svg fill="#f59e0b">` and, separately, a `<div style="color:#fff">` outside any SVG
- **THEN** Gate 1 passes the SVG paint literal but still rejects the `<div>` color literal

### Requirement: Two diagram authoring paths
The tool SHALL support two ways to author diagrams: standard diagrams via the
renderer's supported Mermaid types with automatic layout, and custom/spatial
diagrams (matrices, before/after panels,
swimlanes, layered or grouped regions) via themed HTML/CSS/SVG using
diagram-primitive helper classes. The supported Mermaid types SHALL be those the
Rust renderer can draw; a type outside that set SHALL be rejected by Gate 1
rather than deferred to a client-side renderer, so a page never depends on
shipping a layout engine to the browser. Both paths SHALL carry the stable element
identity contract (`data-id` on nodes) and SHALL theme through the shadcn tokens. For
the Mermaid path the renderer emits `data-id` automatically; for the themed-HTML path
the **agent** SHALL author `data-id` on each element intended to receive feedback, and
Gate 1 SHALL require a `data-id` on any annotatable element of a custom diagram so a
human annotation can be keyed to it.

#### Scenario: Standard graph uses a supported Mermaid type
- **WHEN** the agent needs a diagram of a type the renderer supports
- **THEN** it authors a Mermaid diagram block rendered with automatic layout, and the renderer emits each node's `data-id`

#### Scenario: An unsupported diagram type is rejected rather than deferred
- **WHEN** the agent authors a Mermaid diagram of a type the renderer cannot draw
- **THEN** Gate 1 fails and names the supported types, because falling back to a client renderer would reintroduce the download the static page exists to avoid

#### Scenario: Spatial layout uses themed HTML with authored identity
- **WHEN** the agent needs a 2-D before/after comparison or a matrix that Mermaid's auto-layout cannot place well
- **THEN** it authors a themed HTML/CSS diagram using diagram-primitive helper classes and the shadcn tokens, placing a `data-id` on each element a human may annotate

#### Scenario: Missing identity on an annotatable custom element is rejected
- **WHEN** a custom-diagram element is marked to receive feedback but carries no `data-id`
- **THEN** Gate 1 fails, because an annotation could not be keyed to it

### Requirement: Polished and sketch aesthetics
The renderer SHALL support a polished aesthetic (clean typography and crisp SVG)
and a sketch aesthetic (hand-drawn outlines and sketch font), selectable per
artifact. The sketch aesthetic SHALL apply consistently to Mermaid diagrams,
themed HTML content, and custom diagrams so an artifact reads as one coherent
draft.

#### Scenario: Sketch mode restyles the whole artifact
- **WHEN** an artifact is set to the sketch aesthetic
- **THEN** its Mermaid diagrams, themed content, and custom diagrams all render hand-drawn, signaling a provisional draft

### Requirement: Theme definition and live switching
A theme SHALL be defined by a small set of seed colors from which the full
shadcn-token palette is derived, and SHALL be exposed as CSS custom properties on a root element
so that changing the active theme restyles the entire artifact without re-rendering
any block. The palette SHALL be emitted with the page rather than derived in the
browser, so the document is correctly themed before any script runs. The agent
SHALL be able to define or generate a theme by naming its seed colors.

#### Scenario: Switching a theme needs no re-render
- **WHEN** the active theme's seed colors change
- **THEN** every themed block and diagram restyles via the CSS cascade with no block re-render

#### Scenario: The page is themed without scripting
- **WHEN** a page is opened with JavaScript disabled
- **THEN** it renders in its theme, because the token values were emitted with the document

#### Scenario: Agent generates a theme from seeds
- **WHEN** the agent defines a theme with background/foreground/accent seeds
- **THEN** the renderer derives the remaining shadcn tokens and the theme is selectable

### Requirement: Theme preview on representative content
The tool SHALL let the agent present multiple candidate themes applied to the same
representative content so a human can compare them side by side and select one. The
selection (or an annotation on a theme) SHALL route back to the agent through the
`feedback-transport` channel.

#### Scenario: Compare themes on real-alike content
- **WHEN** the agent presents three candidate themes applied to the same content sampler
- **THEN** the human sees the same content rendered under each theme side by side
- **AND** selecting one routes that choice back to the agent as feedback

### Requirement: Inline SVG is safe image-class content
Gate 1 SHALL allow a **curated static subset** of inline SVG within an `html` block —
`svg`, `g`, `defs`, `title`, `desc`, `path`, `rect`, `circle`, `ellipse`, `line`,
`polyline`, `polygon`, `text`, `tspan`, `linearGradient`, `radialGradient`, `stop`,
`clipPath`, `mask` — so vector art (logos, icons, illustrations, custom diagrams) can
be authored **inline** rather than as an opaque `data:` `<img>` blob, keeping the
artifact source legible and iterable for the agent and the human. Gate 1 SHALL NOT
whitelist the script-bearing or HTML-embedding elements — `script` (already rejected),
`foreignObject`, `style`, and the SMIL animation elements
(`animate`/`animateTransform`/`animateMotion`/`set`/`mpath`) — nor, in this version,
the external-reference elements (`use`, `image`, `textPath`, filter primitives);
any of these SHALL be rejected as a disallowed tag. The existing generic checks —
`<script>` by name, `on*` event-handler attributes, and unsafe URL schemes on
`href`/`xlink:href` — SHALL continue to apply inside the SVG subtree. Consistent with
the `block-validation` capability, the served/baked page CSP (`script-src` without
`unsafe-inline`) SHALL remain the enforcing HTML-safety boundary and the SVG subset a
deterministic fast-fail hint. Inline SVG SHALL be image-class: exempt from the
themed-content color/font/positioning rules, so it MAY use fixed paint. An SVG element
MAY use `fill="currentColor"` (a permitted theme-neutral keyword) to adapt to the
active theme, but SHALL NOT be required to.

#### Scenario: A curated inline SVG logo passes
- **WHEN** an `html` block contains `<svg viewBox="0 0 96 96"><path d="…" fill="#f5f5f4"/><rect fill="#f59e0b"/></svg>`
- **THEN** Gate 1 passes it, because every element is in the curated subset and paint literals are image-class

#### Scenario: Script-bearing SVG is rejected
- **WHEN** an inline SVG contains `<script>`, an `onload=`/`onclick=` handler, or an `xlink:href="javascript:…"`
- **THEN** Gate 1 fails, because those checks apply inside SVG exactly as in HTML

#### Scenario: HTML-embedding SVG element is rejected
- **WHEN** an inline SVG contains `<foreignObject>` or `<animate>`
- **THEN** Gate 1 fails naming the disallowed tag, because those elements are not in the curated subset

#### Scenario: currentColor adapts to the theme
- **WHEN** an inline SVG mark uses `fill="currentColor"`
- **THEN** Gate 1 passes it and the mark inherits the themed foreground, flipping with the active theme

### Requirement: Prose renders GFM from the semantic token kit
Every GitHub Flavored Markdown element the prose renderer can emit SHALL be styled,
and SHALL derive its color from the existing semantic token kit
(`background`, `foreground`, `card`, `muted-foreground`, `border`, `primary`,
`primary-foreground`). No new token SHALL be introduced, so every existing theme —
and any agent-authored `theme` block — themes the prose elements without change.

The styled set SHALL cover headings at every level (`h1`–`h6`), ordered and unordered
lists, blockquotes, thematic breaks, strikethrough, fenced code, inline code, images,
links, and tables. Fenced code in prose SHALL present identically to a `code` block,
and a prose table SHALL present identically to a `table` block, so the prose/block
choice is an affordance decision and not a visual one.

Every rule SHALL be verified across the theme × mode matrix, not in a single mode.

#### Scenario: A GFM element themes with the page
- **WHEN** the reviewer switches mode or picks a theme
- **THEN** every prose element — headings, lists, blockquote, rule, table, code — re-colors with the page, with no hardcoded color left behind

#### Scenario: Prose code and a code block look the same
- **WHEN** an artifact contains a ` ```rust ` fence in prose and a ` ```code lang=rust ` block
- **THEN** the two render with the same surface, border, and type treatment

#### Scenario: A blockquote is distinguishable from a note
- **WHEN** an artifact contains a prose blockquote and a `note` block
- **THEN** the note carries its kind label and annotate affordance while the blockquote is visibly quieter and unlabelled, so the addressable one is not mistaken for the decorative one

### Requirement: Box-drawing art tiles
A code block whose content contains box-drawing or block-element characters SHALL be
rendered with leading tight enough that those glyphs meet across the line boundary, so
a drawn border reads as one continuous stroke rather than a column of dashes. This
SHALL apply to both code forms — a `code` block and a fenced code block in prose — and
SHALL be detected from the content, requiring no declaration by the author.

Ordinary code SHALL keep its looser, more readable leading; only blocks that actually
contain such glyphs are tightened.

A code block's own font and leading SHALL be set on the block element itself, not only
on the inner inline element, because a block's line box is at least as tall as its own
strut — a block left at the inherited body font holds the lines apart no matter what
the inline element asks for.

#### Scenario: A drawn border is continuous
- **WHEN** a code block contains a box drawn with `┌ ─ ┐ │ └ ┘`
- **THEN** the vertical strokes meet across every line boundary and the border renders as an unbroken rectangle

#### Scenario: Ordinary code keeps its reading leading
- **WHEN** a code block contains source with no box-drawing characters
- **THEN** it is not tightened, and keeps the leading used for ordinary code

#### Scenario: Detection needs no authoring change
- **WHEN** an author writes box art in a plain fenced block with no special attribute
- **THEN** it is detected from the content and tiled correctly

### Requirement: Tabular content scrolls rather than overflowing
A table SHALL be wrapped in a horizontal scroll container, whether it comes from prose
or from a `table` block, so that a table wider than the artifact column scrolls
internally instead of escaping the column.

The render gate SHALL treat internally-scrolling content as satisfying its overflow
rule, and SHALL include implicit prose in the set of audited content — prose that can
hold a table can also overflow, so excluding it from the audit would leave the rule
unenforced exactly where it is now reachable.

#### Scenario: A wide table scrolls
- **WHEN** a table's natural width exceeds the artifact column
- **THEN** it scrolls horizontally within its container, the column layout is unchanged, and the render gate reports no overflow finding

#### Scenario: Prose is audited
- **WHEN** implicit prose contains content that escapes the column and does not scroll
- **THEN** the render gate reports an overflow finding for it

### Requirement: Unresolvable links render as hinted text
A link whose target cannot be resolved within a single-artifact page SHALL render as
non-navigable text that discloses its target, rather than as a dead anchor or as
literal markdown source. This SHALL apply to wiki links (`[[target]]`) and to
relative links to another document (for example `other-doc.md`, with or without a
fragment).

Such a link SHALL render its label as text, SHALL carry a visual cue distinguishing it
from both plain prose and a working link, and SHALL disclose its target on hover.

A link with an absolute `http(s)` or `mailto` target SHALL remain a working navigable
link. A link with any other scheme SHALL NOT become an anchor.

#### Scenario: A wiki link discloses its target
- **WHEN** prose contains `[[c4-containers]]`
- **THEN** it renders as the text `c4-containers` with a distinguishing cue, discloses `c4-containers` on hover, and is not clickable

#### Scenario: A cross-document link is hinted, not broken
- **WHEN** prose contains `[Containers](c4-containers.md)`
- **THEN** it renders as the text `Containers` with a distinguishing cue and discloses `c4-containers.md` on hover
- **AND** it does not render as literal `[Containers](c4-containers.md)` source text

#### Scenario: An external link still works
- **WHEN** prose contains `[C4 model](https://c4model.com/)`
- **THEN** it renders as a navigable link in the primary color

### Requirement: Headings are addressable and a table of contents is offered
Every prose heading SHALL carry a stable, GitHub-compatible slug id derived from its
text, so a fragment link to a heading resolves within the page. Colliding slugs SHALL
be disambiguated deterministically.

The viewer SHALL offer a table of contents built from the heading tree in document
order, merged with any block carrying a `title` attribute so that titled diagrams and
tables are reachable alongside prose sections. The table of contents SHALL be
positioned so it does not reduce the artifact column's measure at wide viewports and
SHALL collapse at narrow ones. It SHALL NOT be rendered for an artifact with fewer
than two entries.

#### Scenario: A fragment link resolves to a heading
- **WHEN** prose contains a heading `## The blind spot this fixes` and a link to `#the-blind-spot-this-fixes`
- **THEN** the heading carries that id and the link scrolls to it

#### Scenario: Duplicate headings get distinct ids
- **WHEN** an artifact contains two headings with identical text
- **THEN** each receives a distinct id and both remain individually addressable

#### Scenario: A titled block appears in the table of contents
- **WHEN** an artifact contains ` ```mermaid #d1 title=Containers `
- **THEN** `Containers` appears as a table-of-contents entry in document order and navigates to that block

#### Scenario: A short artifact gets no table of contents
- **WHEN** an artifact has one heading and no titled blocks
- **THEN** no table of contents is rendered

