# Layered graph layout

## Why

Twenty-three diagram types now render in Rust. Four do not — flowchart, class,
er and architecture — and they are 64 of the gallery's 117 diagrams, including
every `graph`, every `stateDiagram-v2` (which is the flowchart pipeline under
another header) and every class diagram.

They are blocked on one thing. The twenty-three that landed have layouts that
are coordinate mappings: a scale, a grid, a squarified treemap, a day axis. You
compute where each thing goes. These four ask a different question — *where
should these nodes go so the edges cross least?* — and the answer was 1.6 MB of
bundled ELK.

The cost of not answering it in Rust is not the megabyte. It is that the project
carries two rendering pipelines: a dynamic import, a second theming path, a
second set of identity contracts, and a Gate 2 that still needs a headless
browser for the types the browser renders. Every change to the visual system has
to be made twice, and the two drift.

`rust-static-renderer` named this change and gated it: *"Deleting it is a
follow-up change, gated on layered layout existing in Rust."*

## What Changes

- **A layered graph layout engine, as a module of its own.** Sugiyama's method:
  break cycles, assign layers, order within layers to reduce crossings, place
  nodes, route edges orthogonally. It knows nothing about diagrams — it takes
  boxes and arrows with sizes and gives back coordinates — so it is testable
  without rendering anything.
- **The four remaining types port onto it**, in the order that pays: flowchart
  first (29 diagrams, and it brings stateDiagram with it), then class, er and
  architecture.
- **The JavaScript renderer and `elkjs` are deleted.** The dynamic import goes,
  the legacy theming path goes, and the renderer stops having a "legacy type
  set" to report.
- **Gate 2 stops needing a browser at all.** The constraint checker already
  reports edges through nodes, merged runs, occluded labels and anything off the
  canvas; it becomes the gate for all 117 rather than for 53.

**Verification changes, and this is the point to be clear about.** The
twenty-three ported types were checked by semantic diff against the renderer
they replaced — 62 cases identical, primitive for primitive. That is not
available here. A layered layout has no single right answer: ELK's coordinates
come from its own heuristics, its own tie-breaks and its own compaction, and
reproducing them exactly is a much larger job than writing a good engine, for a
result no reader could tell apart. So these four are verified by what the
drawing must satisfy rather than by what the old one happened to produce:
the constraint checker, property tests over the engine, and side-by-side review.

We trade *identical* for *provably legible*, and only for these four.

## Capabilities

- `diagram-rendering` — the native set becomes every supported type; the legacy
  renderer and its type set are removed.

## Impact

**Added.** A layout module, and the four types' parsers, layouts and renderers.

**Removed.** `elkjs`, the Preact-era diagram renderer, the dynamic import that
loaded it, and the browser step in Gate 2.

**Not breaking.** Every artifact that renders today still renders. Diagrams of
these four types will not be pixel-identical to their previous output — that is
the intended consequence, not a regression, and the constraint checker is what
holds the line.
