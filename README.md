# carve-wasm 0.2 development branch

This branch binds the unreleased Carve 0.2 Rust engine to WebAssembly for Node and browsers. The Rust engine is pinned to an exact commit on its `next2` branch. There is no 0.2 registry release yet.

Building this branch requires Rust 1.88 or newer.

## Use

Build the package locally with `npm run build`. Node can load the generated package synchronously. Browser modules initialize the web entry first.

```js
import init, { render, parse, lint } from './pkg/web/carve_wasm.js'
await init()
const html = render('# Hello')
const tree = parse('# Hello')
const diagnostics = lint('# Hello')
```

`render` returns HTML. `parse` returns a 0.2 tree, and `lint` returns the 0.2 diagnostic record. The raw JSON boundary is available as `call(requestJson)` and returns an engine response with either `ok` or `error`. See [the API reference](docs/reference.md).

The render default is secure: `rawHtml` is false and the engine applies its URL policy. A trusted host may explicitly set `{ rawHtml: true }`. The `mode`, `profile`, `headingIds`, `symbols`, `labels`, `social`, `sections`, `smartTypography`, `sourceLines`, `roundtrip`, `strictLosses`, and `urlPolicy` options follow Carve 0.2.

## Pending engine capabilities

`toCarve` and `format` run the canonical writer, and `importDocument(html, 'html')` imports HTML. The Markdown, plain text and ANSI writer entry points remain exposed; they use the 0.2 writer registry and report that the writer is unavailable until the Rust engine registers it. The `extensions` option selects the engine's standard extensions, which `extensions()` lists (`carve.tabs`, `carve.wiki`, `carve.cite`, `carve.imgFence.img`, `carve.imgFence.image`), and `renderers: { name: 'standard' }` turns on a standard renderer. Host extension code has no form on the JSON boundary (C2-T-010). The original 0.1 importer, editor, AST patch, merge, trace, stamp and ProseMirror contracts are not part of the 0.2 engine API. The local UTF-8 source patch utility remains available.

See [building](docs/building.md), [compatibility](docs/compatibility.md), and [the exact engine pin](docs/engine-pin.md).
