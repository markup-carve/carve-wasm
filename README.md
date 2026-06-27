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

```js
import init, { toHtml } from '@markup-carve/carve-wasm'

await init()
const html = toHtml('# Hello, Carve!')
document.body.innerHTML = html
```

### Full renderer (extensions on)

`toHtmlFull` enables the same set of extensions as the playground: tab
normalisation, `<details>` fences, Mermaid diagrams, wikilinks, autolink,
list-table, math blocks, heading permalinks, citations, code callouts, and
external-link decoration.

```js
import init, { toHtmlFull } from '@markup-carve/carve-wasm'

await init()
const html = toHtmlFull('# Hello\n\n``` mermaid\ngraph TD; A-->B\n```\n')
document.body.innerHTML = html
```

### TypeScript

The package ships `.d.ts` declarations. Types are inferred automatically when
imported from `@markup-carve/carve-wasm`.

```ts
import init, { toHtml, toHtmlFull, version } from '@markup-carve/carve-wasm'

await init()
console.log(`carve-wasm v${version()}`)
const html: string = toHtml('_Hello_')
```

## API

| Export | Signature | Description |
|--------|-----------|-------------|
| `toHtml` | `(source: string) => string` | Core renderer, no extensions |
| `toHtmlFull` | `(source: string) => string` | Core + common extensions (matches playground) |
| `version` | `() => string` | Returns the carve-rs version string |

## Build

```bash
cargo test
wasm-pack build --target web --scope markup-carve
```

`wasm-pack` emits the browser package into `pkg/`.

To build the Node.js target instead:

```bash
wasm-pack build --target nodejs --scope markup-carve
```
