# Design

## The engine is a module of its own, under `ags-mermaid`

`layout/` takes boxes with sizes and arrows between them, and returns
coordinates. Nothing under it names a diagram, a theme, a label or an SVG.

The reason to keep it apart at all is testability. A layout bug shows up as a
picture, which is the worst possible place to debug one. Held separate, the
engine's properties are assertable directly — every node inside its parent, no
two nodes overlapping, every edge monotone in the layout direction — without
rendering anything and without a diagram type in the way.

It was briefly a sixth workspace crate, and that was wrong. Every other crate
here is a shipping artifact; this one would have been an internal library with
exactly one consumer, and the argument that a separate crate keeps unused layout
out of a WebAssembly build does not survive contact with link-time optimisation,
which strips unreachable code inside a crate too. A module gets the separation
for the cost of a directory.

## What the engine has to do, and nothing more

The scope is not "reimplement ELK". It is "answer the options the renderer
actually asks for", which `layout-engine.ts` states in one place:

| Asked for | What it means here |
|---|---|
| `elk.algorithm: layered` | Sugiyama. No radial, force, stress or tree algorithms. |
| `elk.direction` | DOWN, RIGHT, UP, LEFT — one transposition over one implementation. |
| `elk.edgeRouting: ORTHOGONAL` | Axis-aligned polylines. No splines. |
| `spacing.nodeNode`, `layered.spacing.nodeNodeBetweenLayers` | Within-layer and between-layer gaps. |
| `spacing.edgeEdge`, `edgeEdgeBetweenLayers`, `edgeNodeBetweenLayers` | Routing lanes. |
| `nodePlacement.bk.fixedAlignment: BALANCED` | Brandes–Köpf, averaging its four candidate alignments. |
| `hierarchyHandling: INCLUDE_CHILDREN` / `SEPARATE` | Subgraphs laid out with the parent, or on their own when they override direction. |
| `edgeLabels.inline: true`, `placement: CENTER` | An edge label is a node on the edge, not an annotation beside it. |
| `considerModelOrder: NODES_AND_EDGES` | Source order breaks ties, so a diagram does not reshuffle when nothing changed. |
| `compaction.postCompaction` | Close the slack left after placement. |

Everything else ELK offers is out of scope until something asks for it.

## The five passes

1. **Break cycles.** Depth-first, reversing the back edges found. Reversed
   edges are restored at the end so the drawn arrowhead points where the source
   said it did.
2. **Assign layers.** Longest-path, then tighten: pull each node as late as its
   successors allow, which shortens edges without adding layers. Edges spanning
   more than one layer get chain dummies, so every edge is between adjacent
   layers by the time ordering runs.
3. **Order within layers.** Median heuristic swept down and up, then adjacent
   transposition while it still helps, keeping the best order seen. Source order
   is the tie-break at every step — this is what `considerModelOrder` buys, and
   it is why the same source lays out the same way twice.
4. **Place.** Brandes–Köpf: four alignments (up/down × left/right), each
   compacted along its block graph, then averaged. Dummy chains come out
   straight, which is the property that makes long edges readable.
5. **Route.** Each edge is a polyline through its dummies' coordinates,
   orthogonalised and assigned a lane between layers so parallel edges do not
   merge into one line. The existing constraint checker already reports merged
   runs, so this pass has a test that fails loudly.

## Determinism is a requirement, not a nicety

The same source must produce the same coordinates on every machine and every
run. That rules out iteration order over a hash map anywhere in the engine, and
it is why source order is the tie-break rather than an arbitrary one. Two runs
producing different pictures would make the constraint checker's output
unreproducible and review worthless.

## How this is verified, given parity is gone

Three layers, in increasing cost:

1. **Property tests over the engine.** Assertions that hold for every input,
   checked over generated graphs: no node overlaps another, every node is inside
   its parent's box, every edge is monotone in the layout direction once cycles
   are restored, the same input twice gives the same output.
2. **The constraint checker over every gallery diagram.** It already reports
   edges through unrelated boxes, edges merged into one line, occluded labels
   and anything outside the canvas — the exact failure modes a bad layered
   layout produces. It runs on 53 diagrams today; it runs on all 117 after.
3. **Side-by-side review** of the 64 diagrams against their ELK output. Not to
   match it, but to catch what neither of the above can express: a drawing that
   satisfies every constraint and is still hard to read.

## Ordering

Flowchart first. It carries `graph`, `flowchart` and `stateDiagram-v2` — 29 of
the 64 — and it is the type whose layout options are the richest, so an engine
that satisfies it satisfies the other three. Class and er are the same engine
with a different box; architecture wants ports, which is the one feature it
asks for that the others do not.

`elkjs` comes out only when all four are ported. Until then both paths ship, and
the dynamic import already added in `rust-static-renderer` keeps that free for
artifacts that use none of them.

## What a subgraph frame costs, and one attempt that did not pay for itself

`cicd-pipeline` draws the `ci` frame round `Deploy Staging`, which the source
puts outside it. The cause is not the ordering pass, which was the first guess.
It is the layering: `ci` holds `Fix & Retry`, and `Fix & Retry` is fed from a
node far down the flow, so longest-path layering puts it at the bottom and the
frame — drawn round wherever the members landed — spans everything in between.
No reordering within a layer can fix that.

The fix is hierarchical: lay a group out on its own and place it in its parent
as a single box, which is what ELK does with compound nodes and what the
renderer this replaces therefore got right.

That was attempted and reverted. What it established, so the next attempt starts
from here rather than from the same guess:

- **The placement half works.** Recursing over the container tree — each group
  laid out by its own call to the engine, then placed as one box — makes every
  frame correct by construction, and it makes a group's own `direction` free,
  because that is just the same call with a different direction. `cicd-pipeline`
  came out clean.
- **The routing half does not, naively.** An edge crossing a boundary is routed
  between the *boxes*, so it stops on the frame; carrying it the rest of the way
  to the node with a two-segment elbow is not good enough. Three edges into a
  group of three stacked nodes all enter from one point and run along each
  other. Entering by the face nearest the arrival point helps and is not enough:
  over the corpus the attempt traded one bad diagram for two.
- **What it needs is boundary ports.** A group laid out on its own has to know
  about the edges that will cross it, as a zero-size dummy in its own graph with
  an internal edge to the real node. Then the group's own layered pass routes
  the last leg properly, and the parent only has to reach the frame — the join
  falls inside the padding band, where nothing is drawn.

### The second attempt, which built the ports and still did not pay

Ports were then built as described above, and reverted too. Measured over the
117-diagram gallery with the constraint checker: **16 findings in 7 diagrams
before, 52 in 12 after.** What it settles, so a third attempt does not start
here either:

- **The placement half works, again.** Every frame came out correct by
  construction — `cicd-pipeline`'s `Enclosed` violation went away, nested frames
  sat inside their parents, and a group's own `direction` was free. That half is
  not in doubt and does not need proving a third time.
- **Ports in the child's graph do not meet the parent's routing.** The child's
  own ordering pass decides where its port lands. The parent, which knows the
  child only as a box, routes to whichever face position routing between boxes
  gives it. The two are unrelated, so every crossing edge steps sideways at the
  frame.
- **The seam cannot be repaired afterwards.** Sliding the outer run onto the
  port's coordinate — keeping its direction and taking the port's other axis —
  brought 55 findings down to 52 and produced diagonals where a run had only two
  points. Stitching two independently-routed polylines is not orthogonal
  routing, and no amount of post-hoc geometry makes it one.
- **A two-pass order does not exist.** Constraining the port to where the parent
  will arrive needs the parent's layout first; sizing the parent's box needs the
  child's layout first. There is no ordering that breaks that, which is the real
  reason this keeps failing at the same place.

**So this is an engine change, not an adapter change.** `layout::` has to learn
that a node can carry ports at fixed positions on its boundary, and route to them
rather than to a face — the same thing ELK does. Everything above it is then
straightforward, and the placement half is already known to work.

### The third attempt: the join is solved, and the obstacle moved

`layout::Port` was then built — a caller can pin either end of an edge to a
fraction of the node's side, and it ships. The hierarchical layout was rebuilt on
top of it, with the child deciding the fraction and the parent obeying, which is
the direction that works: the child's layout runs first, so its answer exists,
whereas constraining the child to the parent needs the parent's layout first and
the parent's box size needs the child's.

**The join stopped being the problem.** Crossing wires came out orthogonal, met
their ports on one line, and reached the node rather than the frame. Two real bugs
were found and fixed on the way, both worth knowing about:

- Ordering the pieces of a wire by depth is wrong. A piece from inside the
  *source* end runs out of its container and belongs first; one from inside the
  *target* end runs in and belongs last. Sorting by depth alone puts both extremes
  together and the wire runs backwards through its own drawing.
- A container chain ends at the drawing, so "is `owner` in the chain" is true at
  the root for *every* edge. Every edge wholly inside a group was given a spurious
  port and a second piece that ran off across the page. The test has to be "below
  the container that routes this edge", not "somewhere in the chain". This one
  alone was worth 28 of the findings.

That took the gallery from 55 findings to **27, against a baseline of 16** — and
the `Enclosed` violation went away. Still a net loss, so it was reverted again,
but the remaining gap is no longer routing:

**A group inside a cycle is laid out backwards.** `cicd-pipeline` has `F --> D`
where `D` is in the `ci` group and `F` is downstream of it. Flat, that is one edge
among many. Hierarchically the group is a single node, so it becomes a back-edge
*into* the group, the cycle break turns it, and the layering puts `Deploy Staging`
and `QA Approved?` above the pipeline they come after. The frame is right and the
drawing reads backwards, which is a worse trade than the one it fixes.

That is where a fourth attempt starts: the parent's cycle handling has to
understand that a box standing for a group is not an ordinary node.

### The fourth attempt: everything structural is solved, routing is not

Reverted too, at **13 findings against a baseline of 6** — but the trend is
55 → 27 → 13, and what is left is now one thing rather than four.

The cycle problem was not in the cycle breaker. It was the **order the members are
given to it**. The walk turns round whichever edge closes a cycle *from where it
started*, so a container that lists its groups after its nodes starts the walk
halfway down the flow and calls a forward edge a back edge. Ordering members by
where their contents begin — which is what the flat layout did, because there a
group *was* its nodes — puts `ci` first and the break lands on `F --> D`, which is
the edge that actually closes the cycle.

One dead end worth not repeating: preferring in-degree-zero nodes as walk roots
looks like the general fix and is not. A boundary port has no in-edges, so it
becomes the preferred root of its own group and inverts the group's interior. Root
order must stay index order; it is the *caller's* member order that carries the
meaning.

So all three structural properties now hold together: frames enclose exactly their
members, the drawing reads forwards, and a group's interior reads forwards. What
remains is **routing quality on the port legs**, and only in the two subgraph
diagrams — `cicd-pipeline` and `subgraph-direction-override` account for all eight
of the extra findings, as overlapping runs, a wrong departure face, and one leg
that travels 116px away from its target before turning back.

That is a smaller problem than any previous attempt was left with, and it is
squarely in `route`: a port is a zero-size node, so the spread that keeps ordinary
ends apart has nothing to spread, and the leg from a port gets no lane of its own.

Until that is built, the defect is reported rather than fixed: a frame carries a
`holds` datum and the checker's `Enclosed` rule asks whether the drawing agrees
with it. A wrong drawing that says so is worth more than a wrong drawing that
does not.

## The last six findings were three causes, and one of them was this one

Taking the remaining findings one diagram at a time was the wrong shape: five
diagrams, six findings, and three distinct causes cutting across them. Grouped by
cause instead, two were not defects at all and the third was the port-leg problem
above, in a diagram with no subgraph in it.

**Two were a predicate that only held for orthogonal routing.** `run_hits` asked
whether the run's bounding box met the box's, which is exact while every run is
axis-aligned and wrong the moment one is not: a diagonal fills a sliver of the
box it spans, so comparing the *boxes* reports it as hitting everything in the
corner it reaches across. The requirement diagram joins its boxes with straight
diagonals and the git graph's merge is a cubic, and both were reported passing
through a box the line misses by tens of pixels. Clipping the run against the box
gives the same answer for an axis-aligned run — the four comparisons are the
degenerate case — and the right one for the rest.

There is a second approximation stacked underneath, worth knowing about even
though it did not have to be fixed: `runs` reduces a cubic to its chord, because
`seg_point` keeps only the segment's endpoint. For the git graph the chord misses
hotfix too, so exactness in the predicate was enough. A curve whose chord cuts a
box its arc avoids would still be misreported.

**One was a tree drawing its own trunk.** Every connector under a folder starts
at the same point below its glyph and peels off at its own row; the stem is
shared by design and reads as a stem. The exemption is deliberately the shared
*point* and not "these two edges share a box" — two edges off one node's face are
pushed apart by `route::spread` precisely so they read as two, and excusing every
pair with a node in common would have disarmed the rule that catches it when that
spreading fails. Which is not hypothetical: that is the third cause, below.

**Two were a port leg with no room reserved for it, in a state diagram.**
`place::separation` keeps dummy columns `spacing.edge` apart, and the state
diagram's two columns came out exactly 12px apart, correctly. Then `spread`
offset an arrival port 7px off its node's centre — *after* placement had finished
deciding what fits — and dropped the leg into the middle of that reserved gap,
5px from one column. Under the 6px at which two lines stop reading as two, so the
checker called them one edge.

The fix is the same sentence as the subgraph blocker, and it is why the two are
one problem: **a port leg is not a node, so nothing reserves room for it.** Since
routing runs after placement, the columns are known by the time the ports are
chosen, so the leg can be stepped off them — the nearest position on the face
standing `spacing.edge` clear of every foreign column, which is exact rather than
a search increment, because a position is only ever blocked by a column and so
the nearest free one is hard against a column or against the end of the face.
Three drawings in the gallery moved, all of them further apart than before: the
er diagram's `has` edge went from 7px of clearance to 12, one pixel from having
been a finding of its own.

That leaves the family half-solved and says so: a leg now clears *columns*, and
does not yet clear another node's *leg*, because the two nodes spread
independently and neither knows about the other.

### The fifth attempt, which landed

`flowchart::nest`. **1 finding against a baseline of 2** — the trend across the
five attempts, each against its own baseline, is 52/16, 27/16, 13/6, 13/6, 1/2.
The `Enclosed` violation is gone and cannot come back: a frame is drawn round a
box that *is* the group, so enclosing a stranger is no longer expressible.

Two things made the difference over the fourth attempt, and neither was in the
nesting itself. The first is the port-leg work underneath it — a leg now clears
the columns crossing its gap, and a gap is sized for its lanes — which is what
attempt four's eight extra findings had been. The second was new:

- **Which *side* of a child its port comes out on is the parent's business, and
  the parent can answer it before either is laid out.** The child places its port
  by its own layering, which knows nothing of where the parent's wire will
  arrive; get it wrong and the wire leaves by the face pointing away and travels
  back round. But layering needs only the topology — sizes move boxes within a
  layer, never between layers — so a parent can break its own cycles, assign its
  own layers, and tell each child which way each wire runs, all before a single
  size exists. `descending` is that pass, and it is exact rather than a guess.

And one thing the plan had recorded as free that was not:

- **A group overriding the direction runs across its parent's grain, and then a
  port is the wrong tool entirely.** A port is a node in the child's layered
  pass, so an LR child puts it on a left or right face — which a TD parent's
  engine cannot attach to at all. The wire went round the outside of the frame to
  reach it. Such an edge therefore aims at the *node* instead: the parent pins to
  the fraction where the node sits, and the child's whole piece is the straight
  run from its own boundary in through the header band. Which boundary comes from
  the same `descending` answer — reading the nearest side off the node's position
  instead sends the wire past it and back up into its far face.

Three traps from the earlier attempts had to be kept clear of, and two of them
bit again on the way, exactly as recorded: pieces ordered by travel side rather
than depth, and the port test. That test needed to be **strictly below the
router** — testing chain membership is true at the drawing for every edge, and
testing it downward is true at every group between the router and the endpoint,
which gave an edge wholly inside a child a spurious port in its parent and a
second piece running off across the page. `nested-subgraphs` showed it as `A → B`
carrying a stray leg while `E → A` never reached `A` at all.

A pleasing confirmation: three separate fixtures across the workspace used this
diagram as *the* example of one that draws fine and reads wrong. All three failed
once nesting landed, because it now reads right. They were repointed at the
forced crossing, which is not going the same way.

### Leg versus leg does not exist; lane versus lane does

The obvious next step looked like teaching a leg to clear *another node's* leg,
since two nodes spread their ports independently. Measuring first killed it: over
the whole gallery, at a 12px threshold, there is **not one pair of near-merged
runs belonging to edges that share no node**. The hazard is imaginary.

What the same measurement did find is 13 pairs sitting between 6px — where the
checker starts calling two runs one line — and the 12px `spacing.edge` says two
runs sharing a gap are kept apart by. Their spacings were 7.0, 7.4, 8.3, 9.3:
`gap ÷ (lanes + 1)`. `lanes` places its runs at `(i + 1) / (n + 1)` of the gap and
never once consults `spacing.edge`, so a gap carrying five runs packs them 8.3
apart, and one carrying seven would put them under the width at which they stop
reading as two edges at all.

So the gap is now sized for what crosses it: `max(spacing.layer, (n + 1) ×
spacing.edge)`. That needs the lane count before the layer heights exist, which
sounds circular and is not — a port is chosen from the node's column and its
width and never from a layer's height, so routing can be asked the question
against provisional heights and gives exactly the answer it will later route.
Hence `heights` computes `layer_tops` twice on purpose.

**13 near-merges down to 7, for 0.1% more drawing height overall.** Only three
drawings changed, because `max(50, (n+1) × 12)` does nothing until a gap carries
four runs — the cost falls only where the crowding is.

The 7 that remain all belong to edges that *share a node*, and are `spread`
running out of face: `step` is `width / (count + 1)` capped at `PORT_SPACING`,
with no floor, so a narrow box with three edges puts its ports 7px apart. Fixing
that means widening the box, which is the diagram adapter's business rather than
the engine's, and legs converging on one small box read as converging — the same
argument that excuses a tree's trunk. Left, deliberately.

A note for whoever sizes gaps next: the lane count is every sideways run in the
gap, including runs at opposite ends of the drawing that could have shared a lane.
Colouring the runs as intervals would cut the count. It is not worth it at 0.1%.

### The finding that is left is forced, and proving it is cheaper than chasing it

The state diagram still reports one crossing, and it cannot be routed away. In
the gap, `Connected → Disconnecting` enters at x=93.9 and leaves at 232.7;
`Reconnecting → Closed` enters at 244.7 and leaves at 93.9. The entry order and
the exit order are inverted, so the two connections cross whatever heights their
lanes are given. Nor can the ports escape it: the exit of one is confined to
Disconnecting's face, which begins at 198, and the exit of the other to
Reconnecting's, which ends at 136.7 — the exit order is fixed by the boxes.
Swapping the lanes only converts the crossing into a graze, where the two runs
touch at a shared coordinate and merge for 12px, which is worse.

So it is the ordering pass's crossing to remove, not the router's, and one
crossing in a six-state machine is a plausible optimum rather than a defect. It
is reported, and left.
