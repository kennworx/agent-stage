//! Laying a subgraph out as a unit, so its frame cannot enclose a stranger.
//!
//! The flat layout hands every node to one layered pass and draws each frame
//! round wherever its members landed. Nothing keeps a stranger out of that
//! rectangle, and nothing can: `cicd-pipeline` puts `Fix & Retry` in the `ci`
//! group and feeds it from far down the flow, so longest-path layering drops it
//! to the bottom and the frame spans everything in between.
//!
//! So a group is laid out by its own call to the engine and placed in its parent
//! as a single box. Then a frame encloses exactly its members by construction,
//! and a group's own `direction` is free — it is the same call with a different
//! one.
//!
//! What that costs is edges crossing a boundary, and the answer is a **port**: a
//! zero-size node in the child's own graph, joined to the real endpoint by an
//! edge the child routes itself. The child runs first, so the child chooses where
//! its port sits and the parent obeys, pinning its end of the wire to the same
//! fraction of the child's side. The two pieces then meet on one line, inside the
//! padding band where nothing is drawn.
//!
//! Four earlier attempts are recorded in the change's design; the three traps
//! they found, all of which this has to keep clear of:
//!
//! - **Order the pieces of a wire by which end they came from, never by depth.**
//!   A piece from inside the source runs outward and belongs first; one from
//!   inside the target runs inward and belongs last. Sorting by depth alone puts
//!   both extremes together and the wire runs backwards through itself.
//! - **A port belongs only to a container *below* the one routing the edge.**
//!   Every container chain ends at the drawing, so "is the router in this chain"
//!   is true at the root for every edge — which gave every edge wholly inside a
//!   group a spurious port and a second piece running off across the page.
//! - **Give the members to the engine in the order their contents begin.** The
//!   cycle break turns round whichever edge closes a cycle *from where the walk
//!   started*, so listing groups after nodes starts it halfway down the flow and
//!   calls a forward edge a back edge.
//!
//! Import-only per the workspace conventions.

mod run;
mod tree;
mod wire;

pub use run::layout;
