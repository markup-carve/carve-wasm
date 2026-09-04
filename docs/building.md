# Building carve-wasm

The published package uses the **bundler** target (matching the release
workflow):

```bash
cargo test
wasm-pack build --target bundler --scope markup-carve
```

`wasm-pack` emits the package into `pkg/`. For a no-bundler / `<script type=module>`
setup use `--target web` (which exports a default `init()` you must `await`
before calling the renderers); for Node use `--target nodejs`.

Point the test suite at any built package with `CARVE_WASM_PKG`, rather than
only at `pkg/`:

```sh
CARVE_WASM_PKG=some/other/pkg node tests/smoke.mjs
```

Both wasm-pack targets load. Unset, it is `pkg/`, so nothing changes for the
ordinary loop.

## Rendering-only browser build

All capabilities are enabled by default, so ordinary builds and the published
package keep their complete API. A browser host that only renders Carve to HTML
can leave the importers, reports, AST serialization, and other output formats
out of its artifact:

```sh
wasm-pack build --target web --release --out-dir pkg-render . --no-default-features
```

The basic HTML rendering exports, including `toHtml`, `toHtmlFull`, and
`toHtmlWithOptions`, remain available. Checked rendering through
`toHtmlWithReport` is part of the omitted `reports` capability. The optional
features are:

| Feature | Exports |
| --- | --- |
| `html-import` | `htmlToCarve`, `fromHtml` |
| `markdown-import` | `fromMarkdown` |
| `reports` | all five `*WithReport` renderers |
| `ast-json` | `parseJson`, `astJsonToHtml`, `astJsonToCarve` |
| `other-renderers` | the basic Markdown, plain-text, ANSI, and Carve renderers |
| `lint` | `lintCarve` |
| `stamp` | `readStamp`, `needsReview` |
| `other-imports` | `fromDjot`, `fromBbcode` |

Use a separate output directory as above if a full build also exists in the
checkout.
