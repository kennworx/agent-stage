# Retire the browser-renderer requirements

## Why

Three requirements in `diagram-rendering` describe a renderer that no longer
exists. `elkjs` and the TypeScript diagram renderer were deleted when the Rust
layout engine landed, and the spec was never caught up — so it now says both that
the CLI **shall not** lay out diagrams and that it **shall**, in the same file.

A spec that contradicts itself cannot be used to settle an argument about what
the tool does, which is the only thing a spec is for. Worse, two of the three
would fail if anyone tried to satisfy them: there is no engine bundle to lazy-load
and no TypeScript renderer to diff a port against.

## What Changes

- Remove **Diagrams render in the browser** — superseded by *Diagrams render in
  Rust*, which says the opposite.
- Remove **Bake vs client-render modes** — `mode` was a client-render attribute
  and is no longer in the block vocabulary; there is no bundle to ship or skip.
- Remove **Ported diagram types match the current renderer** — the predecessor it
  diffs against is gone. The 117-diagram byte comparison guards the renderer now.
- Fill in the capability's `Purpose`, which has been a `TBD` placeholder since the
  first archive.

No code changes: this is the spec catching up with code that already shipped.
