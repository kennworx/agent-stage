# Using agent-stage in a web page

`index.html` is a complete, working page: type an artifact, get the rendered
document — prose, diagrams, questions, tables, notes — with no network calls and no
other JavaScript library. It exercises every entry point the module exports: the
whole artifact in one call, each block type on its own, and one sample of every
diagram type the engine draws. `main.ts` is type-checked against the declarations
wasm-bindgen generates, so a renamed export breaks at compile time.

`samples.ts` is generated from `examples/diagram-gallery.md`, so every diagram on
the page is a source the renderer is already tested against. The page checks that
list against `diagram_kinds()` and says so when a type has no sample.

## Build and run

```sh
rm -rf crates/ags-wasm/examples/web/pkg   # wasm-bindgen adds to a directory, it never clears it
cargo build -p ags-wasm --target wasm32-unknown-unknown --profile wasm-release
wasm-bindgen ~/.cargo/target/wasm32-unknown-unknown/wasm-release/ags_wasm.wasm \
  --target web --out-dir crates/ags-wasm/examples/web/pkg
wasm-opt -Oz --all-features --strip-debug --strip-producers \
  crates/ags-wasm/examples/web/pkg/ags_wasm_bg.wasm \
  -o crates/ags-wasm/examples/web/pkg/ags_wasm_bg.wasm
npx tsc -p crates/ags-wasm/examples/web        # main.ts -> main.js
python3 -m http.server -d crates/ags-wasm/examples/web
```

`wasm-bindgen-cli` and `binaryen` supply the middle two commands
(`cargo install wasm-bindgen-cli`, `brew install binaryen`); TypeScript supplies
the fourth. `wasm-pack build --target web` replaces the first three if you would
rather install that instead.

Then open <http://localhost:8000>. Serving over HTTP is not optional — ES modules
and `WebAssembly.instantiateStreaming` both need a real origin, so opening the file
directly gives you a blank page and a console error about the module scheme.

## Size

The module is **1190 KB, or 491 KB gzipped** — decimal KB, the same units your
browser's network panel uses. (On disk `ls` counts in KiB, so it reports a
smaller number for the same file.) What matters is the compressed number, since
that is what crosses the wire.

Binding the whole renderer rather than the diagram engine alone costs about 469 KB
of that — comrak and the page assembly. If a page only ever needs diagrams, a
build exposing `render_svg` alone comes to 721 KB (308 KB gzipped).

The profile does the rest: `wasm-release` in the workspace `Cargo.toml` builds for
size rather than speed. Measured over the 117-diagram gallery on the diagram-only
build, with byte-identical output at every level:

| `opt-level` | after `wasm-opt -Oz` | gzip | per diagram |
| --- | --- | --- | --- |
| `z` *(used)* | 721 KB | 308 KB | ~0.25 ms |
| `s` | 909 KB | 371 KB | ~0.24 ms |
| `2` | 1018 KB | 405 KB | ~0.19 ms |
| `3` *(plain `--release`)* | 1080 KB | 424 KB | ~0.20 ms |

Treat the timings as a range, not a ranking: they moved by as much as 16% between
runs, and level 3 came out slower than level 2 on the second pass. The honest
summary is that every level renders a diagram in roughly a quarter of a
millisecond, so a page with ten diagrams spans about 0.6 ms across the entire
table — while `z` saves 116 KB on every single load. Skipping `wasm-opt` costs
another 90 KB.

Most of what remains is the renderer itself. `render_svg` dispatches over all 27
diagram types, so every one is reachable and none can be dropped by the linker —
a page that only ever draws flowcharts still ships the Gantt renderer. Narrowing
that would mean exposing per-type entry points or a feature per family.

## The API

```js
import init, { render_named_page, render_block, render_svg } from './pkg/ags_wasm.js';

await init();                       // loads the .wasm; nothing works before this
const page = render_named_page(artifact, 'artifact.md');
```

Everything is a string in and a string out. `init()` first — nothing works before
the module is instantiated.

**A whole document.** `render_page(source)` and `render_named_page(source, name)`
return a complete HTML page, styles and chrome included. Put it in an `<iframe
srcdoc>` and you are done.

**One block.** `render_block(fence)` takes a fenced block, fence line included,
and reads the type off it. `render_block_of(type, body, attrs)` is told the type
instead. The typed wrappers — `render_mermaid`, `render_table`, `render_note`,
`render_question`, `render_code`, `render_html` — are for when the call site knows
which it wants. These return *content markup with no `<style>`*, so a page
embedding them needs `block_styles()` in its head once: without it a question
shows list bullets *and* radio buttons, a note loses its rule, a table its
borders.

**One diagram.** `render_svg(source)` returns the bare SVG in token mode (see
below). `render_svg_themed(source, bg, fg, accent)` returns it with literal
colours, for anywhere the cascade cannot be trusted to reach the drawing.

**Checking and listing.** `validate(source)` runs Gate 1 and returns the problems
as TOON, empty when there are none. `catalog()` returns the block vocabulary an
agent writes against, and `diagram_kinds()` every diagram type the engine draws.
`theme_styles(name, body, mode)` resolves a `theme` block into CSS declarations.

The two diagram entry points **throw a string** when the source names no diagram
they can draw — `unknown diagram type 'flowcart' — did you mean 'flowchart'?`. A
thrown string rather than a panic is the point: a panic crossing the WebAssembly
boundary aborts the instance and takes the embedding page down with it, so the
binding converts every failure into a message before it crosses.

## Your page owns the colours

This is the one thing that catches people, so it is worth stating plainly.

`render_svg` draws in **token mode**. Every colour in the output is written as a
CSS variable reference — `fill:var(--_node-fill)` — and those derive, through
`color-mix()`, from three variables the *page* defines:

```css
:root { --ags-bg: #ffffff; --ags-fg: #1e2430; --ags-accent: #3b82f6; }
```

Define them and the diagram inherits your theme. Leave them out and every `fill`
resolves to nothing, which browsers paint as black: a diagram of solid black boxes
with no visible text is what a missing `--ags-fg` looks like.

The payoff is that a theme change costs nothing. Flip `--ags-bg` and `--ags-fg` — on a
media query, a class, a toggle — and every diagram on the page restyles through the
cascade, with no re-render and no second call into WebAssembly. `index.html` does
exactly that: the Mode switch flips `data-theme` on `<html>` and every diagram on
the page follows, without a second call into WebAssembly.

Five further variables override an individual blend if you dislike it:
`--ags-muted`, `--ags-line`, `--ags-surface`, `--ags-border`, `--ags-accent`.

The `ags-` prefix is not decoration. Custom properties inherit, so a page that
defines its own `--border` would have that value win inside every diagram it
embeds, over the engine's fallback — which is exactly what used to happen.

## Embedding the drawing somewhere with no cascade

If the SVG is going somewhere that cannot resolve a variable — an `<img src>`, an
email, a file on disk — token mode is the wrong mode, because there is no page to
answer the reference. The Rust API takes `ColorMode::Fixed` for that case and
writes literal colours throughout; the `ags draw` subcommand does it by default.
This binding does not expose the choice, since a diagram being drawn *into a page*
is the case it exists for.
