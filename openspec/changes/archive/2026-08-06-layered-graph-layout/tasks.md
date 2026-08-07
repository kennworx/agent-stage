# Tasks

## 1. The engine, standalone

- [x] Create `ags-mermaid/src/layout`: boxes with sizes, edges between them, a direction, spacings — coordinates out. No diagram, theme or SVG concepts under it
- [x] Break cycles depth-first, recording the reversed edges so they can be restored
- [x] Assign layers by longest path, then tighten so no edge is longer than it needs to be
- [x] Insert chain dummies so every edge spans exactly one layer
- [x] Order within layers: median sweep down and up, then transposition, keeping the best order seen
- [x] Break every tie by source order, so the same input gives the same output
- [x] Place with Brandes–Köpf, averaging its four alignments
- [x] Compact after placement to close the slack
- [x] Route edges orthogonally through their dummies, in lanes so parallel edges do not merge
- [x] Restore reversed edges, so an arrowhead points where the source said

## 2. Prove the engine before drawing anything with it

- [x] Property test: no two nodes overlap
- [x] Property test: every node is inside its parent's box — the canvas, the engine having no nesting of its own
- [x] Property test: every edge is monotone in the layout direction once cycles are restored
- [x] Property test: the same input twice gives byte-identical output
- [x] Test each of the four directions is the same layout transposed
- [x] Test a graph of one node, of none, and one that is entirely a cycle

## 3. Subgraphs

- [x] Teach `layout::` that a node can carry ports at fixed positions on its boundary, and route to them rather than to a face — `layout::Port`, shipped and tested. Unused by the renderer until the task below lands
- [x] **Landed, on the fifth attempt.** A subgraph is laid out by its own call to the engine and placed in its parent as one box, so a frame encloses exactly its members by construction — `flowchart::nest`. The four earlier attempts measured 52, 27, 13 and 13 findings against baselines of 16, 16, 6 and 6; this one measures **1 against 2**, and the one left is the forced crossing in `state-connection-lifecycle`. What made the difference over attempt four: the two port-leg fixes underneath it (a leg clears the columns crossing its gap; a gap is sized for its lanes), plus the parent working out which *side* of a child its wire will arrive at before the child places its port

- [x] Lay out a subgraph with its parent, so edges cross the boundary
- [x] Lay out a subgraph on its own when it overrides the direction, then place the result as one box — free, as predicted, except for one thing that was not: a group running across its parent's grain puts its port on a face the parent's engine cannot attach to at all, so such an edge aims at the node itself instead of at a port
- [x] Reserve the header band so a subgraph's title does not sit on its first row

## 4. Flowchart — 29 diagrams, and it brings stateDiagram

- [x] Port the flowchart parser: nodes, twelve shapes, edge styles, labels, subgraphs, styling
- [x] Read `stateDiagram-v2` onto the same structure
- [x] Port the layout adapter onto `ags-layout`
- [x] Port the renderer, including edge labels as nodes on the edge
- [x] Clip each edge to the shape it points at rather than to its bounding box
- [x] Run the constraint checker over all 29 and fix what it reports
- [x] Review side by side against the ELK output

## 5. Class, er, architecture

- [x] Port class: parser, layout adapter, renderer, and its member compartments
- [x] Port er: parser, layout adapter, renderer, and its crow's-foot ends
- [x] Port architecture: parser, renderer, and its own grid placement — its side letters say where a thing goes, not where a line leaves, so the layered engine is the wrong tool and it does not use it
- [x] Run the constraint checker over all 35 and fix what it reports
- [x] Review side by side against the ELK output — moot: ELK is gone, so there is
  no second output to compare against. The 117-diagram byte comparison is what
  guards the renderer now

## 6. Remove the JavaScript renderer

- [x] Delete `elkjs` and the legacy diagram renderer
- [x] Remove the dynamic import and the legacy theming path
- [x] Remove the renderer's legacy type set; the native set becomes the supported set
- [x] Drop the headless-browser step from Gate 2 entirely — Gate 2 no longer
  exists; Gate 1 draws every diagram, so there is nothing left for a browser to
  find
- [x] Confirm `examples/reasoning-demo.md` renders — `ags present --check` passes,
  which now draws every diagram. Its `mode=live` half is moot: `mode` was a
  client-render attribute and is no longer in the block vocabulary
- [x] Re-measure the page-weight budget with a graph-bearing artifact — the demo
  bakes to 47 KB (13 KB gzipped); the 117-diagram gallery to 1.09 MB (99 KB
  gzipped). Nothing is fetched, so that is the whole cost
