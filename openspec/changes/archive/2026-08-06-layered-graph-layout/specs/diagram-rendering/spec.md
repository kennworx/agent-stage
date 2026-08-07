# diagram-rendering

## ADDED Requirements

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

### Requirement: The render gate launches no browser

Render checks SHALL be geometry assertions over the scene the renderer computes,
for every diagram type. The gate SHALL NOT launch a browser.

#### Scenario: A graph diagram is checked in the gate
- **WHEN** a flowchart, class, er or architecture diagram is rendered
- **THEN** the gate reports edges passing through unrelated nodes, edges merged into one line, occluded labels and anything outside the canvas, without a browser
