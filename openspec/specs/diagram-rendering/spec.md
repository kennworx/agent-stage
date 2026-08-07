# diagram-rendering Specification

## Purpose
Drawing the mermaid-family diagrams an artifact declares, natively in Rust, and
checking the geometry that comes out. Rendering is a pure function from source to
SVG, so the same text draws the same picture on every machine — and legibility is
asserted rather than eyeballed.
## Requirements
### Requirement: Renderer must emit stable element identity
The tool SHALL depend on the diagram renderer emitting stable, source-derived element
identity — each node wrapped in `<g class="node" data-id="<source id>">` and each edge
carrying `data-from`/`data-to` — because annotation anchoring (`feedback-transport`),
dependency-highlight, and simulation overlays all key off it. A renderer that does not
provide this contract SHALL NOT be usable as the tool's engine. (The contract itself is
defined and tested in the beautiful-mermaid renderer's own spec; this requirement makes
the tool's dependency on it explicit.)

#### Scenario: Missing node identity fails the engine requirement
- **WHEN** a candidate renderer emits diagram nodes with no stable `data-id`
- **THEN** it does not satisfy this requirement and cannot back the tool's interactivity

### Requirement: Diagrams render in Rust

A diagram block of a natively supported type SHALL be parsed, laid out and
emitted as SVG by the Rust renderer, with no browser or JavaScript engine
involved.

#### Scenario: A diagram block becomes inline SVG

- **WHEN** an artifact containing a diagram block is served or baked
- **THEN** the page contains the finished `<svg>` for that diagram inline
- **AND** producing it required no browser

#### Scenario: Rendering is deterministic

- **WHEN** the same diagram source is rendered twice with the same options
- **THEN** the two SVG outputs are byte-identical

A layout that shifts between renders is worse than a mediocre one that holds
still: it makes review diffs meaningless and defeats caching of baked output.
"Same options" is load-bearing rather than pedantic — a host may supply its own
text measurement, and geometry derives from it, so determinism holds per measurer
and every port-verification diff must pin the default one.

### Requirement: Rendering is a pure function from text to text

The renderer SHALL expose rendering as diagram text in, SVG text out. It MUST NOT
read a filesystem, consult a clock, or start a thread, and MUST NOT panic on any
input. Malformed input SHALL return an error identifying what and where.

The same code has to run on a server and in a browser: `ags` renders server-side,
while other consumers — a server that is not Rust, or a page where the diagram is
editable — render in the browser. A panic there aborts the WebAssembly instance
and takes the page with it, so "returns an error" is a hard requirement rather
than a courtesy.

#### Scenario: Malformed input yields an error, not a crash

- **WHEN** the renderer is given a diagram whose body it cannot parse
- **THEN** it returns an error naming the problem and the line
- **AND** does not panic

#### Scenario: The same renderer runs in a browser

- **WHEN** the renderer is compiled for WebAssembly and given no measurement callback
- **THEN** it renders a diagram to SVG requiring no imports beyond memory

#### Scenario: A host may supply text measurement

- **WHEN** the host provides its own text measurement
- **THEN** the renderer uses it in place of the built-in estimate
- **AND** it is the only host facility the module requires

### Requirement: Constraint violations are reported, never silently accepted

Where a rendered diagram violates a legibility invariant, the renderer SHALL
still produce the drawing and SHALL report the violations alongside it.

The two callers need opposite things from the same result. An editor rendering as
an author types needs the drawing regardless, and would otherwise blank on every
intermediate keystroke. Returning both leaves the decision with the caller instead
of guessing on their behalf.

A violation SHALL NOT by itself refuse service. It describes a drawing that exists
and reads badly, which is a **finding for the agent** — the `finding` kind the
feedback channel already carries — not grounds to deny a human the review.

> This requirement originally said `ags` treats any violation as a gate failure.
> That was written before the checks had been run over a real corpus. Measured
> against `examples/diagram-gallery.md`: **15 of 117 diagrams violate, 308
> violations in total**, and 7 of the 15 are xychart, where an axis line crossing a
> plot box is reported as an edge passing through a node. Enforcing the original
> rule would refuse this project's own example artifact, and most of what it
> refused would be checker calibration rather than a defect a reader would notice.
> Gating on a measure this noisy teaches an author to distrust the gate.

#### Scenario: A violating diagram still renders

- **WHEN** a diagram is rendered whose geometry breaks an invariant
- **THEN** the SVG is produced
- **AND** the violation is reported with the elements involved

#### Scenario: A violating diagram is still served

- **WHEN** an artifact contains a diagram with reported violations, and nothing else fails
- **THEN** Gate 1 passes and the review is served
- **AND** the violations are carried to the agent as findings, not shown as an error

#### Scenario: A block's violations arrive as one finding

- **WHEN** one diagram block reports several violations
- **THEN** they are recorded as a single finding against that block, listing each
- **AND** re-presenting the unchanged artifact records nothing further

#### Scenario: A redrawn diagram retires its finding

- **WHEN** an artifact is presented again and a diagram that previously reported
  violations now reports none
- **THEN** its recorded finding is retracted, so a later poll does not return it

### Requirement: A closed outline is an area, not a route

The route legibility rules SHALL apply only to shapes that are not closed.

A closed outline returns to where it began. Every question these rules ask about a
route — does it double back, does it leave by the face pointing away — is therefore
answered "yes" by the shape itself rather than by the drawing. A sankey link is the
case: an edge with a source and a target, drawn as a filled ribbon running out along
one side and back along the other. All six backtracking findings over the reference
gallery were one such diagram.

Closure is the test. Not the shape kind — a ribbon is a `Path`, the same variant an
orthogonal route uses — and not the paint, since a ribbon takes its fill from a CSS
class rather than from the scene.

#### Scenario: A ribbon is not asked whether it doubles back

- **WHEN** a diagram draws a link as a closed outline between two boxes
- **THEN** no backtracking, crossing, or attachment-face finding is reported for it

### Requirement: A stroke that connects nothing is not judged as an edge

The edge legibility rules SHALL apply only to strokes that declare an endpoint.

Both rules ask a question that presupposes a connection: *passes through a box it
does not connect*, and *two connections read as one wire*. A stroke carrying
neither a source nor a target is a chart series, an axis, a spine — it connects
nothing, so every box it crosses is one it "does not connect" and every crossing is
reported. Over the reference gallery this was 286 findings, **not one of which
named its edge**, because a stroke with no identity is the same stroke with no
endpoints.

#### Scenario: A chart series may cross the bars beneath it

- **WHEN** a diagram draws a line series over boxes it declares no connection to
- **THEN** no crossing is reported

#### Scenario: An edge with an endpoint is still judged

- **WHEN** a stroke declares a source or a target
- **THEN** a box it crosses that it does not connect is reported as before

### Requirement: Supported diagram types are declared

The renderer SHALL expose the diagram types it draws as the single source of
truth. Gate 1 SHALL reject a type it cannot draw, naming the supported set, and
the block catalog's diagram-type list SHALL be generated from that same set.

The artifact format already works from a closed block vocabulary the validator
owns; extending it to diagram types keeps the two from drifting. An author should
learn at authoring time that a type does not exist, not have a reader discover a
blank figure.

#### Scenario: An unknown type is rejected at Gate 1

- **WHEN** an artifact declares a diagram of a type the renderer does not support
- **THEN** validation fails, naming the type and listing the supported ones
- **AND** nothing is served

#### Scenario: A near miss is named as one

- **WHEN** the declared type is one edit from a supported one
- **THEN** the failure suggests it, by the same Damerau rule block types use

#### Scenario: The catalog reflects the renderer

- **WHEN** the block catalog is printed
- **THEN** its diagram-type list is generated from the renderer's supported set,
  so the two cannot drift

### Requirement: Colour follows the target, and CSS derives wherever CSS exists

A diagram rendered into a page SHALL express every fill and stroke as a CSS
custom property reference carrying a literal fallback, and SHALL emit CSS to
derive any colour the theme does not name rather than computing it. A diagram
rendered as a standalone image SHALL carry literal colours throughout.

Inside a page, deriving in CSS means the result follows *any* token change — the
page can restyle every diagram by changing the accent alone, with no re-render,
which is what allows light/dark switching to work with no script. Computing those
values in the renderer would leave a page that overrides the accent with boxes in
the new colour and derived shades still based on the old one, invisible until
someone looked at a chart.

A standalone image has no page and no cascade, so the same reasoning inverts: a
token reference there resolves to nothing. Literal colours are what make a
diagram embeddable somewhere that is not our own document.

#### Scenario: A scheme change restyles a page diagram without re-rendering

- **WHEN** the page's colour tokens change
- **THEN** every diagram rendered into it restyles through the cascade
- **AND** no diagram is re-rendered

#### Scenario: Overriding one token moves everything derived from it

- **WHEN** the page overrides the accent token alone
- **THEN** every colour derived from the accent changes with it, including shades
  the theme never named

#### Scenario: A page diagram opened on its own still renders

- **WHEN** a diagram rendered for a page is opened with nothing defining the tokens
- **THEN** it renders in its fallback colours rather than unstyled

#### Scenario: A standalone image carries no token references

- **WHEN** a diagram is rendered as a standalone image
- **THEN** every colour in it is a literal
- **AND** it renders identically with no stylesheet present

### Requirement: Diagrams render to a raster image deterministically

The renderer SHALL be able to produce a raster image of a diagram, and the same
source and options SHALL produce the same image.

A raster has to draw glyphs rather than name them, so it depends on a font at
render time. Resolving that against whatever fonts a machine happens to have
would make the same diagram rasterise differently in different places, which
defeats comparing images at all — and would silently change an artifact's
appearance depending on where it was built.

#### Scenario: The same source rasterises identically twice

- **WHEN** a diagram is rasterised twice with the same options
- **THEN** the two images are byte-identical

#### Scenario: Rasterisation does not depend on the host's fonts

- **WHEN** the same diagram is rasterised on two machines with different fonts installed
- **THEN** the two images are identical

### Requirement: Rendered diagrams carry element identity

Every diagram element a reviewer may annotate SHALL carry a stable `data-id`
emitted by the renderer, matching the identity the current renderer emits for
that element.

Element-level feedback is keyed to this attribute. A port that produced correct
geometry but dropped the identity would look right and silently break annotation
— and would do so without any visual symptom to catch it.

#### Scenario: Nodes are addressable after the port

- **WHEN** a diagram is rendered by the Rust renderer
- **THEN** each node carries the same `data-id` the current renderer emits for it

#### Scenario: An existing annotation still resolves

- **WHEN** an artifact with recorded element-level feedback is re-rendered by the
  Rust renderer
- **THEN** each annotation still resolves to its element

### Requirement: Rendered geometry satisfies legibility invariants

The renderer SHALL check the geometry it produces and report violations of the
invariants that make a diagram readable: no edge drawn through a box, no two
edges sharing a line, no two edges crossing where a reader tracing one could
leave on the other, no attachment face disagreeing with the direction of
travel, no route doubling back, and nothing drawn outside the canvas.

Every one of these is a defect that shipped during the C4 work while passing
every check then in place. They were found by eye, repeatedly, after being
reported as fixed. The renderer computes the geometry, so it is the only place
that can check it cheaply and exactly.

#### Scenario: A diagram with an overlapping edge pair is reported

- **WHEN** a rendered diagram contains two edges collinear within the merge
  tolerance over a shared span
- **THEN** the check reports the pair, the overlap length, and the diagram

#### Scenario: An edge leaving by the wrong face is reported

- **WHEN** a route leaves a box by the face pointing behind it, away from the box
  it is going to
- **THEN** the check reports the edge and the box it left

#### Scenario: A route that doubles back is reported with how far

- **WHEN** a route travels away from its target beyond the detour tolerance
  before turning toward it
- **THEN** the check reports the edge and the distance travelled backwards

#### Scenario: Edges converging on a shared box are not reported as crossing

- **WHEN** two edges meet at a box they both connect
- **THEN** no crossing is reported, because that is the diagram being connected

#### Scenario: Checks run without a browser

- **WHEN** the render gate runs
- **THEN** it operates on the renderer's own geometry
- **AND** requires no headless browser

### Requirement: Paint order is declared, not implied by emission order

Each element of a rendered diagram SHALL declare the layer it belongs to, and the
renderer SHALL paint layers in a fixed order — frames, then edges, then nodes,
then labels, then overlays. Within a layer, order SHALL follow the scene, so the
same source always paints identically.

SVG has no `z-index`: it paints in document order, so whatever is written first
is covered by everything written after. That turns paint order into an emission
detail every diagram type has to get right independently — and one that fails
silently, because the geometry is correct and only the picture is wrong. A
description bubble in the current renderer was painted over by a badge belonging
to a later step for exactly this reason.

#### Scenario: An overlay is drawn above content emitted after it

- **WHEN** a diagram emits an overlay before other elements
- **THEN** the overlay is painted above them

#### Scenario: Order within a layer follows the scene

- **WHEN** two elements share a layer
- **THEN** they paint in scene order, and re-rendering the same source paints them the same way

#### Scenario: Content hidden behind a later layer is reported

- **WHEN** an element that must be legible is fully covered by something painted after it
- **THEN** the check reports it

### Requirement: Text measurement is shared and exact across types

All diagram types SHALL measure text through one shared primitive, ported to
match the existing character-width model.

#### Scenario: Canvas size derives from the shared metric

- **WHEN** a diagram's canvas is sized from a measured label
- **THEN** the measurement comes from the shared primitive
- **AND** matches the value the current renderer computes for that string

### Requirement: Layered graph layout in Rust

The renderer SHALL lay out node-and-edge diagrams in Rust, with no JavaScript
engine involved. The layout SHALL break cycles, assign layers, order nodes
within layers to reduce crossings, place them, and route edges orthogonally.

#### Scenario: A cyclic graph still lays out
- **WHEN** a source declares edges that form a cycle
- **THEN** the diagram is laid out, and every arrowhead points the way the source declared

#### Scenario: The same source lays out the same way twice
- **WHEN** the same source is rendered twice, on any machine
- **THEN** every coordinate is identical

#### Scenario: A long edge stays readable
- **WHEN** an edge spans more than one layer
- **THEN** it is drawn as a straight run through the layers it crosses rather than as a staircase

### Requirement: Every diagram type renders natively

The renderer SHALL draw every diagram type it accepts. No diagram type SHALL
require a JavaScript renderer.

#### Scenario: A flowchart renders without a browser
- **WHEN** an artifact contains a `graph`, `flowchart` or `stateDiagram-v2` block
- **THEN** the page is complete markup, with no renderer downloaded

#### Scenario: No artifact pulls a legacy chunk
- **WHEN** an artifact of any supported diagram type is validated
- **THEN** it reports no legacy renderer cost, because there is none

### Requirement: A subgraph frame encloses exactly its members

A `subgraph` SHALL be laid out as a unit and placed in its parent as a single
box, so the frame drawn round it contains its own members and no other node. A
group that declares its own `direction` SHALL be laid out in that direction and
still placed as one box.

#### Scenario: A frame does not reach round a stranger
- **WHEN** a group holds a node that is fed from further down the flow, so a flat layering would drop it past nodes outside the group
- **THEN** the frame is drawn round its members only, and the checker reports no enclosure

#### Scenario: An edge crossing a boundary reaches the node
- **WHEN** an edge joins a node inside a group to one outside it
- **THEN** the wire is drawn to the node itself rather than stopping at the frame, and every run of it is axis-aligned

#### Scenario: A group may run across its parent's grain
- **WHEN** a group declares `direction LR` inside a `graph TD`
- **THEN** its contents read left-to-right, the drawing round it still reads top-to-bottom, and the wire between them turns once

### Requirement: Every run between layers keeps the declared clearance

Runs sharing the space between two layers SHALL be kept `spacing.edge` apart,
whether they come from a long edge's own column or from an edge meeting a node's
side. The space between layers SHALL grow to hold the runs crossing it.

#### Scenario: A busy gap does not pack its lanes together
- **WHEN** more edges cross one gap than fit at the declared clearance
- **THEN** the gap is made taller rather than the runs drawn closer

### Requirement: A route is drawn straight wherever its two ends can meet

A wire SHALL be drawn as one straight run wherever its two ends can be brought
onto a single column, rather than paying two corners for a difference nothing
required.

Ports are spread along a box face so that sibling edges do not leave on top of
one another, and where a face is uncontested a port moves to line up with the
other end of its own wire. A port SHALL NOT move past a neighbour on its own
face, and SHALL NOT move onto a column already crossing the gap its leg has to.

Each face was settled on its own, so a wire whose source had three siblings and
whose target had none left off-centre and arrived at the centre. Over a
117-diagram gallery that cost 110 bends and nothing else.

#### Scenario: A wire whose far face is empty is drawn straight
- **WHEN** one end of a wire shares its face with other edges and the other end does not
- **THEN** the uncontested end moves to meet the first, and the wire is drawn with no corner

#### Scenario: A crowded face refuses the move
- **WHEN** the neighbours on a face leave no room between them for a port to move into
- **THEN** the port stays where the spreading put it, and the ordering that decides which wires cross is preserved

#### Scenario: A box is sized for the wires that will meet it
- **WHEN** a cycle break turns a back edge round, so a box has more wires leaving it than the author wrote
- **THEN** its face is sized for the wires that will actually be routed, at the spacing the spreading pass uses

### Requirement: Runs sharing a gap are ordered so they do not cross

Lanes SHALL be assigned so that two runs sharing a gap do not cross wherever an
assignment exists that avoids it.

A run crosses the space between two layers in a lane of its own, entering at one
column and leaving at another. A run entering inside another's span takes the
higher lane; one leaving inside another's span takes the lower. Where both hold
— two runs that nest, or that genuinely swap over — the pair crosses whatever
lane it is given, and the reading order SHALL stand.

Ordering by where a run starts is exactly backwards whenever two runs overlap
without nesting, which is the one arrangement that guarantees the crossing.

#### Scenario: A run entering inside another takes the higher lane
- **WHEN** two runs share a gap and one enters between the columns the other travels between
- **THEN** the entering run is given the lane nearer the upper layer, and neither leg cuts the other

#### Scenario: Two nested runs keep the order they came in
- **WHEN** neither lane assignment avoids the crossing
- **THEN** the runs are left in reading order rather than reordered to no effect

### Requirement: A label sits beside its own line and across no line at all

A label SHALL be placed beside the leg of its own route that it is anchored to,
and SHALL be treated as obstructed by every other line — including the other
legs of its own route.

Exempting the whole route is what drew a verb with its own turn struck through
the middle of the word: an elbow puts the anchor near the corner, so the leg
round that corner is precisely the one in the way. Exempting nothing is worse —
the label leaves the line it names and reads as belonging to no wire at all.

#### Scenario: A label is not drawn across its own corner
- **WHEN** a route turns, and the anchor for its label falls near the turn
- **THEN** the label is placed clear of the leg round the corner, still beside the leg it is anchored to

### Requirement: A label and the wire it names are highlighted together

Pointing at either a wire's label or the wire SHALL raise both. The highlight
SHALL be carried by the emitted stylesheet and SHALL NOT require script, because
these drawings are embedded under a Content-Security-Policy that blocks script
execution. A wire with nothing written on it SHALL carry no highlight at all.

Where a label is a child of its wire's own group the pairing is structural, and a
single rule covers every drawing. Where paint order forces the label to be a
separate top-level node, the pair SHALL be named on both halves and given a rule
of its own.

#### Scenario: Hovering a label raises its wire
- **WHEN** the pointer rests on an edge label
- **THEN** the label, the line and its arrowhead are drawn in the highlight colour

#### Scenario: An unlabelled wire is left alone
- **WHEN** the pointer rests on a wire that carries no label
- **THEN** nothing is highlighted, and the drawing carries no rule that such a wire could match

#### Scenario: The highlight survives a page that forbids script
- **WHEN** a drawing is embedded in a page whose policy blocks inline and external script
- **THEN** the pairing still works, because it is expressed in the stylesheet the drawing carries

### Requirement: A proportion chart is a ring, and every share is legible

A pie SHALL be drawn as a ring rather than a disc, with each slice a sector of
that ring. Every slice's share SHALL be readable somewhere: written in the band
where the slice is wide enough to hold it, and in the legend for every slice
whether or not it is.

A slice too thin to carry its own number is the one a reader most needs the
number for. A share that rounds to nought SHALL be written with enough precision
to be worth reading.

#### Scenario: A slice too thin to label carries its number in the legend
- **WHEN** a slice's share is below the width at which a number fits inside it
- **THEN** the wedge is drawn without a number and the legend gives it

#### Scenario: A single slice is still a ring
- **WHEN** one slice covers the whole circle
- **THEN** it is drawn as a ring with the hole cut, not as a disc

#### Scenario: A vanishing share is not written as nought
- **WHEN** a slice's share is below one per cent
- **THEN** the legend writes it with a decimal rather than rounding it to `0%`

#### Scenario: A wedge is highlighted without being repainted
- **WHEN** the pointer rests on a wedge, its swatch or its legend row
- **THEN** the wedge is outlined and the legend's words take the highlight colour, and the wedge's fill is unchanged because that fill is the datum

### Requirement: The render gate launches no browser

Render checks SHALL be geometry assertions over the scene the renderer computes,
for every diagram type. The gate SHALL NOT launch a browser.

#### Scenario: A graph diagram is checked in the gate
- **WHEN** a flowchart, class, er or architecture diagram is rendered
- **THEN** the gate reports edges passing through unrelated nodes, edges merged into one line, occluded labels and anything outside the canvas, without a browser

