# carve-wasm

WebAssembly build of the [Carve](https://markup-carve.github.io/carve/) markup
renderer, compiled from
[carve-rs](https://github.com/markup-carve/carve-rs). Lets the Rust
implementation run client-side in the browser and backs the **Rust (WASM)**
engine in the [Carve playground](https://markup-carve.github.io/carve/playground).

## Install

```bash
npm install @markup-carve/carve-wasm
```

> Publishing to npm is pending. Until the package is live you can build locally
> (see [Build](#build) below) or consume the `pkg/` output directly.

## Usage

### Core renderer

Renders Carve markup to HTML with no extensions enabled.

The published package is the **bundler** target (webpack, Vite, Rollup, ...):
the wasm initializes automatically, so the exports are synchronous - no `init()`
call.

```js
import { toHtml } from '@markup-carve/carve-wasm'

const html = toHtml('# Hello, Carve!')
document.body.innerHTML = html
```

### Full renderer (extensions on)

`toHtmlFull` enables the same set of extensions as the playground: tab
normalisation, `<details>` fences, Mermaid diagrams, wikilinks, autolink,
list-table, math blocks, heading permalinks, citations, code callouts, and
external-link decoration.

```js
import { toHtmlFull } from '@markup-carve/carve-wasm'

const html = toHtmlFull('# Hello\n\n``` mermaid\ngraph TD; A-->B\n```\n')
document.body.innerHTML = html
```

### Symbols

A `:name:` symbol renders its literal `:name:` source unless the name is in the
**symbols map**. Pass one as a plain object (or a `Map`) to `toHtmlWithSymbols`,
or as the optional second argument of `toHtmlFull`:

```js
import { toHtmlWithSymbols } from '@markup-carve/carve-wasm'

toHtmlWithSymbols('Ship it :rocket:', { rocket: '🚀' })
// => '<p>Ship it 🚀</p>'

toHtmlWithSymbols('Ship it :rocket: :shrug:', { rocket: '🚀' })
// => '<p>Ship it 🚀 :shrug:</p>'   (an unmapped name stays literal)
```

The word-boundary guard is unaffected by an active map: `a:b:c`, `10:30:` and
`me@example.com` never become symbols. Names and values must both be strings; a
non-string value throws a `TypeError`.

> **Security: symbol values are TRUSTED RAW output.**
> A mapped value is inserted into the output **unescaped** - the same trust
> class as a static `renderers` callback. `{ b: '<b>x</b>' }` emits a real
> `<b>` element, not escaped text. This is deliberate (processor configuration
> is trusted). **Never build a symbols map out of untrusted / user-supplied
> input.**

### Section wrappers

A top-level heading is wrapped, along with the content following it up to the
next same-or-shallower heading, in a `<section>` carrying the heading's id (spec
PART 9 §13). Only the id moves - `{#install .featured}` gives
`<section id="install"><h2 class="featured">` - and a heading inside a
blockquote, div or list item is not wrapped at all.

`toHtmlWithOptions` is the general entry point, and `sections: false` renders
headings flat with the id back on the `<h*>`:

```js
import { toHtmlWithOptions } from '@markup-carve/carve-wasm'

toHtmlWithOptions('# A\n\np\n', { sections: false })
// '<h1 id="A">A</h1>\n<p>p</p>'

toHtmlWithOptions(src, { sections: false, symbols: { rocket: '🚀' }, full: true })
```

Every field is optional - `sections` (default `true`), `symbols` (same trusted-raw
contract as `toHtmlWithSymbols`), and `full` (default `false`, enabling the same
extension set as `toHtmlFull`). Omitting the object, or passing `null`, renders
with defaults, so the three shorthands above remain the zero-config forms.

An unrecognized key is ignored, because the object is configuration and a typo
should not break a render. A recognized key with the wrong type throws a
`TypeError` instead of being coerced: JS truthiness would read
`{ sections: 'false' }` as `true`, the opposite of what was written.

This exists for a host whose CSS or JS assumes rendered blocks are direct
children of the content container - the `.stack > * + *` spacing idiom,
`:first-child`, `nth-child()` counting, `element.children` walks - all of which
stop matching once a wrapper sits in between. It is the one output change that
breaks a document whose *source* migrated cleanly.

Nothing else changes: ids, collision dedup, `</#id>` cross-references, implicit
`[Heading][]` references and heading numbering all resolve against the slug
rather than the element carrying it. The endnotes
`<section role="doc-endnotes">` is a separate construct and is still emitted.

### TypeScript

The package ships `.d.ts` declarations. Types are inferred automatically when
imported from `@markup-carve/carve-wasm`.

```ts
import { toHtml, toHtmlFull, version } from '@markup-carve/carve-wasm'

console.log(`carve-wasm v${version()}`)
const html: string = toHtml('_Hello_')
```

## API

| Export | Signature | Description |
|--------|-----------|-------------|
| `toHtml` | `(source: string) => string` | Core renderer, no extensions |
| `toHtmlWithSymbols` | `(source: string, symbols?: object \| null) => string` | Core renderer + a `:name:` -> value symbols map (values are raw, see above) |
| `toHtmlFull` | `(source: string, symbols?: object \| null) => string` | Core + common extensions (matches playground), optional symbols map |
| `toHtmlWithOptions` | `(source: string, options?: object \| null) => string` | General form: `{ sections?, symbols?, full? }`, every field optional |
| `parseJson` | `(source: string) => string` | The parsed AST as JSON (PART 12 exchange shape) |
| `version` | `() => string` | Returns the carve-wasm package version |

### The parsed AST

`parseJson` returns the document as a JSON string - the [PART 12 exchange
shape](https://markup-carve.github.io/carve/ast-json), the same tree every Carve
engine publishes, so a consumer written against one implementation reads
another's output.

```js
import { parseJson } from '@markup-carve/carve-wasm'

const ast = JSON.parse(parseJson('# Title\n\nBody[^a].\n\n[^a]: note\n'))
ast.children.map((n) => n.type) // ['heading', 'paragraph', 'footnote']
ast.children[0].pos             // { startLine: 1, startColumn: 1, ... }
```

The root carries exactly `type`, `children` and `srcByteLength`; frontmatter and
footnote definitions are block nodes inside `children`, not root fields. Every
node except the root carries `pos` when the engine could place it - 1-based
lines and columns, 0-based offsets, ends exclusive, counted in Unicode
**codepoints**, not bytes or UTF-16 units. A node the engine could not place,
such as reassembled table-cell text, carries no `pos` at all rather than an
invented one.

A string rather than a JS object: the caller runs `JSON.parse`, which the
browser does natively and faster than building the object graph across the wasm
boundary one property at a time - and it keeps the bytes available for a caller
that stores or forwards them.

## carve-rs dependency pin

`Cargo.toml` pins an exact carve-rs commit, and `Cargo.lock` is committed
alongside it:

```toml
carve = { package = "carve-lang", git = "https://github.com/markup-carve/carve-rs", rev = "..." }
```

Read the current revision out of `Cargo.toml` rather than from a copy here - a
revision quoted in prose goes stale the first time someone bumps the manifest
without noticing the duplicate.

The engine is published as `carve-lang` (carve-rs renamed it from `carve`), so a
pin at any revision past that rename needs `package = "carve-lang"` as above.

The crate previously tracked carve-rs' default branch with no committed lock.
That never went stale, but it went the other way: every build resolved whatever
had landed upstream since, so the published package could carry an engine no CI
run here had ever built, and two clones a day apart could disagree. The pin
makes an engine change a reviewable line in a diff.

When bumping the `rev`, regenerate and commit `Cargo.lock` in the same change.
The lock records the resolved revision plus the rest of the tree; leaving it
behind gives every fresh clone a dirty working tree on its first build and lets
the package resolve to an engine other than the one that was tested.

```sh
cargo update -p carve-lang --precise <sha>   # or edit the rev and re-lock
cargo test && wasm-pack build --target nodejs && node tests/smoke.mjs
CARVE_SPEC_CORPUS=/path/to/carve/tests/corpus node tests/corpus.mjs
```

That last line is the one that can tell a drifted pin from a current one.
`smoke.mjs` asserts hand-written expectations, which a stale engine satisfies
happily; `corpus.mjs` renders all ~530 mandatory spec documents through the
**built** artifact and requires byte-identical HTML. Without `CARVE_SPEC_CORPUS`
it prints a notice and exits 0, so a checkout without the spec repo still runs
the suite. CI always sets it.

It measures the binding as much as the engine: carve-rs is corpus-checked
upstream, but that says nothing about whether the wasm-bindgen layer drops a
field or mangles an option on the way through.

Regenerate the whole lock MSRV-aware, or it will quietly break the `rust-version`
this crate advertises. A plain `cargo generate-lockfile` on a current toolchain
picks the newest `wasm-bindgen`, which needs a newer Rust than the 1.75 declared
above - fine while CI only runs `stable`, and a hard failure for anyone actually
building on the floor:

```sh
CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo generate-lockfile
```

Nothing in CI catches this today, since the workflow uses `stable` only. Adding
a 1.75 job (carve-rs has one) would turn it from a review question into a build
failure.

## Build

The published package uses the **bundler** target (matching the release
workflow):

```bash
cargo test
wasm-pack build --target bundler --scope markup-carve
```

`wasm-pack` emits the package into `pkg/`. For a no-bundler / `<script type=module>`
setup use `--target web` (which exports a default `init()` you must `await`
before calling the renderers); for Node use `--target nodejs`.
