# diagram-rendering

## REMOVED Requirements

### Requirement: Diagrams render in the browser

**Reason**: Superseded by *Diagrams render in Rust*. The engine it names was
deleted with `elkjs`; the CLI is the renderer, so "the CLI/host SHALL NOT
rasterize or lay out diagrams" is the opposite of what ships.

### Requirement: Bake vs client-render modes

**Reason**: `mode` was a client-render attribute and is no longer in the block
vocabulary. There is no engine bundle to load lazily or to skip, so neither
scenario can be satisfied or violated.

### Requirement: Ported diagram types match the current renderer

**Reason**: The TypeScript renderer the port was to be diffed against is gone,
so there is no second output to compare with. The 117-diagram byte comparison is
what guards the renderer against silent divergence now.
