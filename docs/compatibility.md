# Package compatibility

The release tarball contains these entry points:

| Import | Initialization | Intended host |
| --- | --- | --- |
| `@markup-carve/carve-wasm` | Automatic | Node or a browser bundler with native WASM imports |
| `@markup-carve/carve-wasm/node` | Automatic | Node ESM and CommonJS |
| `@markup-carve/carve-wasm/web` | `await init()` | Browser modules and module workers |
| `@markup-carve/carve-wasm/render` | `await init()` | Browser hosts that only render HTML |
| `@markup-carve/carve-wasm/offsets` | None | UTF-16, UTF-8 and codepoint boundary conversion |

The root export selects the Node entry through the `node` condition. Browser
bundlers select the bundler entry. A bundler that enables Node conditions for a
browser build should use `/web` explicitly.

## Browser modules

Resolve the package through a bundler or serve the package directory, retaining
its hierarchy. The full entry points share the package-root WASM file. Initialization is asynchronous; rendering after it is
synchronous.

```js
import init, { toHtmlWithOptions } from '@markup-carve/carve-wasm/web'
await init()
const html = toHtmlWithOptions(source, { rawHtml: false, profile: 'comment' })
```

The rendering-only entry uses the same pattern. It has no parser JSON,
importers, reports, includes or editor APIs. Both entries export a default
initializer that can take the WASM module or URL when the default relative URL
is unsuitable. Serve `.wasm` as `application/wasm` for streaming compilation.

CI installs the npm tarball and tests Vite 8.3.2 production bundles in
Chromium, Firefox and WebKit through Playwright 1.61.1. Vite's build target is
`esnext` because these entries use top-level await. Earlier Vite releases may
need a WASM plugin for the root bundler entry; they are outside this tested
matrix. This matrix does not establish a minimum browser version.

## Node and TypeScript

Node 22 is tested with both `import` and `require`. The Node build loads its
package-root WASM file synchronously, so copying only its JavaScript file fails.

TypeScript 5.9.3 is tested with `strict: true`, `skipLibCheck: false` and
`NodeNext` module resolution against the installed tarball. wasm-bindgen emits
`Symbol.dispose` declarations for `ParserSession`; include `ESNext.Disposable`
in `compilerOptions.lib` when the rest of the project targets ES2022. Explicit
`session.free()` remains supported.

## Editors and workers

Edit ranges use UTF-8 byte offsets. Browser selection ranges use UTF-16 code
units; AST positions use Unicode codepoints. Convert against the source the
edit applies to, before sending it to a worker. See `/offsets` and the
[incremental API reference](reference.md#re-parsing-while-someone-types).

[carve-worker.mjs](../examples/carve-worker.mjs) initializes `/web` once and
retains one `ParserSession`. Messages carry `id` and `action`: `open` takes
`source`, `edit` takes `changes`, `render` takes `source`, and `close` frees the
session. Replies contain `value` or `error`. The consumer suite tests this
protocol in each browser. Create it with a bundler-resolved module worker:

```js
const worker = new Worker(new URL('./carve-worker.mjs', import.meta.url), {
  type: 'module',
})
```

Copy the example into the application beside this call. Free retained sessions
when documents close, and terminate workers when the host closes them.

## Fidelity and resource limits

HTML import projects through a DOM and cannot retain all Carve source
constructs. Its `roundtrip` mode is for trusted Carve-produced HTML. The corpus
ledger records output differences, while the stability gate requires a second
import to settle. Inspect conversion reports before accepting imported source.
Markdown, Djot and BBCode migration reports still mark fidelity as unverified.

ProseMirror reports dropped nodes, degraded nodes and canonical source changes.
A nonempty report requires a host decision before replacing the original
source. Preserve the original source when an editor cannot represent it.

The engine limits nesting to 200 levels. CI checks that depth in browsers and
measures the Node stack needed by serialization separately. Larger documents
still consume synchronous CPU time; move work into a worker when it would block
an editor. `scripts/benchmark.mjs` measures the shipped package, but its elapsed
times are machine-dependent and are not latency guarantees.
