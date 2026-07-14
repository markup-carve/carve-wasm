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
| `version` | `() => string` | Returns the carve-wasm package version |

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
