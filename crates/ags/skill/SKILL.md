---
name: agent-stage-authoring
description: Author a reasoning artifact for human review with `ags`. Use when you're about to present analysis, a plan, a comparison, or a decision for a human to see, comment on, and approve. Covers deciding when to build one, reading the block catalog, composing to the quality bar, self-review, and treating the artifact as the approval request.
license: MIT
compatibility: Requires the `ags` CLI.
metadata:
  author: agent-stage
  version: "1.0"
---

# Authoring a reasoning artifact

A reasoning artifact is a markdown file a human reviews in the browser with
`ags present` and replies to with `ags poll`: they annotate blocks, answer
questions, and approve. Gate 1 (`ags present --check`) guarantees the artifact is
**correct** — it validates every block *and draws every diagram*, so an artifact
that passes has a picture on every `mermaid` block. This skill is how you make it
**good**. The two are orthogonal — a valid, rendering artifact can still be
padded, abstract, or a menu of undecided options.

## 1. Decide whether to build one

Build an artifact when a human must **see, compare, comment on, or approve**
something — a plan, a design, a comparison, a decision with tradeoffs. Otherwise
answer in chat.

- **Skip the trivial.** A single fact, a yes/no, a one-line answer is a message,
  not an artifact.
- **Never pad.** Every block earns its place. No filler, no restating the prompt.
- **Never ship a single-step artifact.** If it's one block, it's a message.

## 2. Read the catalog before authoring

Run `ags catalog`. It prints the live block vocabulary and each type's schema —
straight from the validator, so it is exactly what Gate 1 accepts. Author against
that output, never from memorized tags: a block or attribute you half-remember is
how you get a rejected artifact.

Its last section lists **every diagram type a `mermaid` block may open with**,
generated from the renderer itself. Check a diagram's header against that list
rather than reaching for one Mermaid supports upstream: the header is what selects
a renderer, and a type outside the list is rejected by Gate 1 — with a suggestion
when it is one edit from a real one, so a typo names itself.

### Fence only what needs an affordance

The catalog is the closed set of **addressable** types — the ones a human can act
on. Everything else is prose, and prose is full GitHub Flavored Markdown:
headings, lists, tables, blockquotes, links, and fenced code all work with no
fence type and no id.

So the question is never "is there a block for this?" but **"does the human need
to do something to it?"** Fence it when they must annotate a diagram node, comment
on a table cell or a code line, answer a question or a claim, or switch a theme.
Otherwise write plain markdown.

Two traps, because both look right and fail silently:

- **` ```rust ` is prose, not a code block.** It renders as plain code and carries
  **no line-annotation affordance**. If you want a reviewer to comment on a
  specific line, you must write ` ```code lang=rust `. Reaching for the
  language-tagged fence out of habit is the easiest way to quietly lose that.
- **A markdown table and a ` ```table ` block render identically.** The only
  difference is that a reviewer can annotate a cell of the block. Use a plain
  table freely — it is not a lesser form — and fence it when you want cell
  comments.

A type one edit from a real one (` ```mermiad `, ` ```tabel `) is rejected rather
than treated as prose, so a typo can't silently degrade into a grey code block.

## 3. Compose to the quality bar

- **Concrete-first.** Lead with one concrete example before any abstraction —
  show the thing, then generalize.
- **Stand alone.** The artifact reads without the chat history. No revision
  language ("unlike the previous version", "as we discussed") — the reader may
  never have seen the prior turn.
- **Preserve altitude.** Keep the core separate from motivating examples and
  adapters. Don't drag a high-level decision down into implementation detail, or
  inflate a detail into a headline.
- **Reuse first.** Before adding a block, name what it reuses — an existing
  diagram, table, or decision. New structure is a cost; justify it.
- **Commit, don't menu.** State settled decisions as settled. Only genuinely-open
  choices are deferred — and they go in **one** Open Questions block at the
  bottom, each with a recommended default. Never scatter open questions through
  the artifact; never offer a menu when you have a recommendation.
- **Verify the real workflow.** Close by exercising the actual path, not a
  restated goal: `ags present --check <file>` to confirm Gate 1 passes, then
  `ags present <file>` to see it render and review.

## 4. Visual discipline

- **No chrome by default.** No decorative boxes, no color for its own sake. Color
  comes only from theme tokens (see the `html` and `theme` types in `ags catalog`).
- **Two diagram paths.** A standard graph relationship (flow, sequence, state) is
  a `mermaid` block (auto-layout; the renderer emits each node's `data-id`). A
  genuinely 2-D or spatial one (before/after, a matrix, swimlanes, layered
  regions) is a themed `html` block built from the renderer's `.ui-diagram-*`
  primitives (`ui-diagram-grid`/`-row`/`-col`, `-node`, `-region` + `-region-title`,
  `-label`, `-arrow`) — colors come from the tokens, so it flips with the theme.
  Prefer a 2-D layout over a long left-to-right chain. **Any `.ui-diagram-node` a
  human might annotate needs a `data-id`** (Gate 1 rejects one without) — that is
  the identity a review comment keys to, exactly like a Mermaid node.
- **Sketch signals a draft.** Use the sketch aesthetic for a provisional artifact
  that invites edits; polished for a settled one.
- **Fix the guidance, not the artifact.** If a human critiques the *look* or
  *structure*, that is a signal about the renderer or this skill — improve the
  shared guidance, don't hand-patch one stored artifact.

## 5. Self-review before handoff (high-stakes only)

For a high-stakes artifact — a decision expensive to get wrong, a plan others
will build on — run **one** skeptical pass whose only job is to find what is
**weak, missing, or wrong**, not to praise. Review the written artifact; do not
re-research.

It is **non-blocking**: present the artifact, let the human start reading, review
concurrently. Fix clear-cut defects in place; route genuine judgment calls to the
Open Questions block — never decide them silently.

## 6. Presenting *is* the approval request

Presenting the artifact is the request for sign-off. Do **not** add a separate
"does this look good?" — the artifact is the question.

- **Read-only until approved.** Don't act on the artifact's plan until the human
  approves it.
- **The artifact is the source of truth.** When scope shifts, update the artifact,
  not the chat. The reviewer's replies (drained by `ags poll`) drive the next
  revision.
