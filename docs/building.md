# Building carve-wasm

Use Node 22.12 or later, Rust with the `wasm32-unknown-unknown` target, and
wasm-pack 0.15.0, matching CI.

```sh
npm ci --ignore-scripts
cargo test --locked
npm run build
```

The build script emits the complete npm package in `pkg/`: bundler bindings at
the root, plus Node, web and rendering-only bindings in subdirectories. It also
copies the host documentation and examples. A direct `wasm-pack build` creates
only one target and does not produce the release package's exports.

## Consumer checks

```sh
npx playwright install --with-deps chromium firefox webkit
npm test
npm run test:consumers
node tests/feature-types.mjs
CARVE_SPEC_CORPUS=/path/to/carve/tests/corpus node scripts/verify-release-artifact.mjs
npm run bench
```

Consumer checks pack and install the tarball before loading it. They test Node
ESM and CommonJS, TypeScript declarations, Vite production bundles, browser
rendering, depth limits and the module worker example. The release gate also
runs the corpus, HTML roundtrip and ProseMirror suites on the unpacked artifact.
Benchmarks write ignored `benchmark-results.json` with startup, rendering,
stateless and retained edits, WASM size, and memory after repeated operations.
They assert paragraph reuse and bounded late memory growth; timing is reported.

Point artifact tests at another package with `CARVE_WASM_PKG`. The build script
accepts `--no-opt` for a faster development build; release sizes must be checked
on the optimized build.

## Local measurements

One optimized wasm-pack 0.15.0 run on Node 22.22.2 produced these results with
10 timing samples. Elapsed times vary by machine and are not release budgets.

| Operation | Median |
| --- | --- |
| Render 250 heading/link blocks | 6.13 ms |
| ProseMirror conversion of those blocks | 39.83 ms |
| Stateless edit of 1,000 plain paragraphs | 24.68 ms |
| Retained edit of those paragraphs | 17.60 ms |

The edit input is 38,889 bytes; retained edits parse 35 bytes. They still clone
the snapshot and serialize the full tree and source layout, so the complete
operation remains proportional to document size. Syntax-bearing documents
fall back to a full parse.

The full WASM payload is 5,870,878 bytes, or 2,109,871 with gzip level 9. The
rendering-only payload is 1,930,387 bytes, or 687,261 gzipped. Bundler, Node and
web bindings share the full file after the build checks byte identity. Late
memory samples during 100 renders and freed sessions stayed at 12,648,448
bytes. Rerun `npm run bench` for the current build.

## Feature selections

All capabilities are enabled by default. The packaged `/render` entry is built
with `--no-default-features` and keeps `toHtml`, `toHtmlFull` and
`toHtmlWithOptions`. Its consumer checks reject editor and importer exports.

| Feature | Capabilities |
| --- | --- |
| `html-import` | HTML-to-source and HTML-to-AST import |
| `markdown-import` | Markdown migration |
| `reports` | Five checked renderers |
| `ast-json` | AST parsing and rendering |
| `ast-merge` | Three-way AST merge |
| `ast-patches` | AST patch creation and replay |
| `includes` | Synchronous host-resolved includes |
| `incremental` | Snapshots, reparsing and `ParserSession` with `ast-json` |
| `other-renderers` | Markdown, plain text, ANSI and Carve rendering |
| `lint` | Source diagnostics |
| `stamp` | Stamp inspection |
| `other-imports` | Djot and BBCode import |
| `prosemirror` | Editor document conversion |
| `source-patches` | Source patch creation and replay |

For another feature selection, use a separate output directory:

```sh
wasm-pack build --target web --release --out-dir pkg-custom . --locked --no-default-features --features reports
```
