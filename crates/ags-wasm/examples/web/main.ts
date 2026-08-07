// Rendering an agent-stage artifact in the browser.
//
// Every entry point the module exports is called from here: a whole page in one
// call, each block type on its own, and one sample of every diagram type. The
// types come from `pkg/ags_wasm.d.ts`, which wasm-bindgen generates next to the
// JavaScript — so `render_page(1)` is a compile error rather than a confusing
// runtime one, and the editor knows what the module exports without being told.

import init, {
  block_styles,
  catalog,
  diagram_kinds,
  render_block,
  render_block_of,
  render_code,
  render_html,
  render_mermaid,
  render_named_page,
  render_note,
  render_question,
  render_svg,
  render_svg_themed,
  render_table,
  theme_styles,
  validate,
} from './pkg/ags_wasm.js';
import { DIAGRAMS } from './samples.js';

/** The elements this page drives, resolved once so a typo fails at startup. */
function need<E extends Element>(id: string): E {
  const found = document.getElementById(id);
  if (!found) throw new Error(`the page is missing #${id}`);
  return found as unknown as E;
}

const source = need<HTMLTextAreaElement>('source');
const mode = need<HTMLSelectElement>('mode');
const palette = need<HTMLSelectElement>('palette');
const preview = need<HTMLIFrameElement>('preview');
const problems = need<HTMLPreElement>('problems');
const vocabulary = need<HTMLPreElement>('vocabulary');
const blocks = need<HTMLElement>('blocks');
const diagrams = need<HTMLElement>('diagrams');
const diagramNote = need<HTMLParagraphElement>('diagram-note');
const coverage = need<HTMLElement>('coverage');

// ---------------------------------------------------------------------------
// Palettes
// ---------------------------------------------------------------------------

/**
 * The palettes the switch offers, each written as a `theme` block body.
 *
 * They are resolved by `theme_styles()` — the same code a served page runs ahead
 * of time — so what you see here is what an agent gets by writing the same three
 * lines into an artifact. A lone `seed:` expands to the whole palette through an
 * OKLCH lightness ramp; the first entry sets nothing and so falls through to the
 * base cascade.
 */
const PALETTES: ReadonlyArray<readonly [string, string]> = [
  ['base', ''],
  ['indigo', 'seed: #6366f1'],
  ['teal', 'seed: #14b8a6'],
  ['amber', 'seed: #f59e0b'],
  ['rose', 'seed: #f43f5e'],
  ['slate', 'seed: #64748b'],
  // Not a seed: two ends given, and the middle tokens filled by `color-mix()`.
  ['paper', 'background: #fbf7ef\nforeground: #2b2622\nprimary: #a8632c'],
];

palette.innerHTML = PALETTES.map(
  ([name]) => `<option value="${name}">${name}</option>`,
).join('');

/**
 * Apply the chosen mode and palette.
 *
 * `base.css` declares `color-scheme: light dark` and defines every token twice,
 * under `:root` and `:root[data-theme='light']`. Setting the attribute is the
 * whole light/dark switch — and declaring both schemes is what stops a browser
 * with "force dark" on from deciding the page is light-only and inverting it,
 * which is how a correct stylesheet ends up looking wrong.
 *
 * The palette rides on top as an inline style, which outranks both `:root` rules
 * without needing a stylesheet of its own.
 */
function applyTheme(): void {
  const [name, body] = PALETTES.find(([n]) => n === palette.value) ?? PALETTES[0]!;
  document.documentElement.dataset['theme'] = mode.value;
  document.documentElement.style.cssText = theme_styles(name, body, mode.value);
}

/**
 * A probe for reading a resolved token as a colour.
 *
 * `getComputedStyle().getPropertyValue('--card')` gives back the token's *text*,
 * which for a mixed token is the `color-mix(…)` expression rather than a colour.
 * Putting the token on a real property and reading that instead makes the browser
 * do the resolving, whatever the token was written as.
 */
const probe = document.createElement('div');
probe.style.cssText =
  'position:absolute;width:0;height:0;visibility:hidden;' +
  'background-color:var(--card);color:var(--foreground);' +
  'border:0 solid var(--primary)';
document.body.append(probe);

/** `rgb(18, 22, 31)` to `#12161f`; anything else through unchanged. */
function toHex(css: string): string {
  const channels = css.match(/\d+(\.\d+)?/g);
  if (!css.startsWith('rgb') || !channels || channels.length < 3) return css;
  return `#${channels
    .slice(0, 3)
    .map((c) => Math.round(Number(c)).toString(16).padStart(2, '0'))
    .join('')}`;
}

/** The three colours a literal-coloured drawing is given, read off the page. */
function diagramPalette(): readonly [string, string, string] {
  const now = getComputedStyle(probe);
  return [toHex(now.backgroundColor), toHex(now.color), toHex(now.borderTopColor)];
}

// ---------------------------------------------------------------------------
// Panels
// ---------------------------------------------------------------------------

/** One labelled panel: the call that produced it, and what it returned. */
function panel(call: string, html: string): string {
  return `<div class="panel"><p class="call">${call}</p>${html}</div>`;
}

/** The body of the first fenced block of `type` in `text`, if there is one. */
function firstBlock(text: string, type: string): string | undefined {
  for (const chunk of text.split('```')) {
    if (!chunk.startsWith(type)) continue;
    return chunk.split('\n').slice(1, -1).join('\n');
  }
  return undefined;
}

/** The gallery sample for a diagram kind, for panels that want their own. */
function sample(kind: string): string {
  return DIAGRAMS.find(([k]) => k === kind)?.[1] ?? 'graph LR\n  A --> B';
}

/**
 * Every block type, each through its own entry point.
 *
 * The closed set is seven: mermaid, question, table, code, html, note, theme. Six
 * of them draw something; `theme` is agent configuration rather than content, so
 * it is the palette switch above rather than a panel.
 *
 * Where the artifact has a block of the type its body is used, so editing the
 * source drives that panel. Everywhere else the content differs deliberately: two
 * panels showing the same drawing tell you nothing about the difference between
 * the two calls that produced it.
 */
function renderBlocks(text: string): string {
  const mermaid = firstBlock(text, 'mermaid') ?? 'graph LR\n  A --> B';
  const question = firstBlock(text, 'question') ?? 'Ship it?\n- yes\n- no';
  const note = firstBlock(text, 'note') ?? 'Worth knowing.';
  const code = firstBlock(text, 'code') ?? 'fn main() {}';
  const table =
    firstBlock(text, 'table') ?? '| a | b |\n| --- | --- |\n| 1 | 2 |';
  const [bg, fg, accent] = diagramPalette();

  const panels = [
    panel('render_mermaid(body) — the block above', render_mermaid(mermaid)),
    panel('render_table(body)', render_table(table)),
    panel("render_note(body, 'claim')", render_note(note, 'claim')),
    panel(
      "render_note(body, 'info')",
      render_note('Rendered blocks carry no `<style>` of their own.', 'info'),
    ),
    panel(
      "render_note(body, 'warn')",
      render_note('Without `block_styles()` this is a skeleton.', 'warn'),
    ),
    panel("render_question(body, 'radio')", render_question(question, 'radio')),
    panel(
      "render_question(body, 'checkbox')",
      render_question(
        'Which gates should run on push?\n- clippy\n- coverage\n- the browser check',
        'checkbox',
      ),
    ),
    panel(
      "render_question(body, 'select')",
      render_question(
        'Which release channel?\n- stable\n- beta\n- nightly',
        'select',
      ),
    ),
    panel(
      "render_question(prompt, 'text')",
      render_question('Anything else worth saying?', 'text'),
    ),
    panel("render_code(body, 'rust')", render_code(code, 'rust')),
    panel(
      'render_html(body)',
      render_html(
        '<p class="ui-lead">Themed HTML: colours come from ' +
          '<code>var(--token)</code>, never a literal.</p>',
      ),
    ),
    // The two general ones. `render_block` reads the type off the fence, so it is
    // given a fence of a type no other panel uses; `render_block_of` is told the
    // type instead, so it is given a body that could have been anything.
    panel(
      'render_block(fence) — type read from the fence',
      render_block('```note #ship kind=warn\nThe fence said `note`, not the call.\n```'),
    ),
    panel(
      "render_block_of('table', body, '') — type given as an argument",
      render_block_of(
        'table',
        '| call | type from |\n| --- | --- |\n| render_block | the fence |\n| render_block_of | the argument |',
        '',
      ),
    ),
    // And the two bare-SVG entry points, which return no <figure> around them.
    // Different diagrams, because what separates these two is the colours: one
    // defers to the page, the other carries its own.
    panel(
      'render_svg(source) — token colours, resolved by this page',
      svgOr(() => render_svg(sample('pie'))),
    ),
    panel(
      'render_svg_themed(source, bg, fg, accent) — literal colours, page ignored',
      svgOr(() => render_svg_themed(sample('sequenceDiagram'), bg, fg, accent)),
    ),
  ];
  return panels.join('');
}

/**
 * Run a renderer that can fail, and show why when it does.
 *
 * The diagram entry points return `Err` rather than panicking — a panic crossing
 * the WebAssembly boundary aborts the instance, which would take down the page —
 * and wasm-bindgen surfaces that as a thrown value.
 */
function svgOr(render: () => string): string {
  try {
    return render();
  } catch (thrown: unknown) {
    const message = String(thrown);
    return `<p class="bad">${message.replace(/[<&]/g, (c) => (c === '<' ? '&lt;' : '&amp;'))}</p>`;
  }
}

/** One sample of every diagram type, each drawn through `render_mermaid`. */
function renderDiagrams(): void {
  diagrams.innerHTML = DIAGRAMS.map(([kind, sample]) =>
    panel(`render_mermaid(…) — ${kind}`, svgOr(() => render_mermaid(sample))),
  ).join('');

  // What the engine says it draws, against what this page has a sample for. The
  // page reports the gap rather than quietly showing 26 of 27.
  const known = diagram_kinds().split('\n').filter(Boolean);
  const shown = new Set(DIAGRAMS.map(([kind]) => kind));
  const missing = known.filter((kind) => !shown.has(kind));
  diagramNote.textContent =
    missing.length === 0
      ? `All ${known.length} types the engine draws, one sample each, from examples/diagram-gallery.md.`
      : `${shown.size} of ${known.length} types — no sample for: ${missing.join(', ')}`;
  diagramNote.classList.toggle('bad', missing.length > 0);
  coverage.textContent = `${known.length} diagram types · ${DIAGRAMS.length} samples`;
}

// ---------------------------------------------------------------------------
// The loop
// ---------------------------------------------------------------------------

/** Gate 1, then draw everything that depends on the source or the theme. */
function refresh(): void {
  const artifact: string = source.value;
  applyTheme();

  const errors: string = validate(artifact);
  problems.textContent = errors || 'Gate 1: no errors.';
  problems.classList.toggle('bad', errors.length > 0);

  preview.srcdoc = render_named_page(artifact, 'artifact.md');
  blocks.innerHTML = renderBlocks(artifact);
}

await init();

// The rules the rendered blocks are written against, and the theme tokens this
// page reads. Without them the markup is a skeleton: a question shows list bullets
// *and* radio buttons, a note loses its rule, a table its borders. `render_page`
// needs none of this — a whole document carries its own — but a block placed into
// someone else's page does.
//
// Prepended, not appended: the page's own chrome rules come after and win where
// the two overlap.
const sheet = document.createElement('style');
sheet.textContent = block_styles();
document.head.prepend(sheet);

// The vocabulary the artifact above is written against — generated from the
// validator, so an editor showing it is showing the rules that are enforced.
vocabulary.textContent = catalog();

refresh();
renderDiagrams();
source.addEventListener('input', refresh);
mode.addEventListener('change', () => {
  refresh();
  renderDiagrams();
});
palette.addEventListener('change', () => {
  refresh();
  renderDiagrams();
});
