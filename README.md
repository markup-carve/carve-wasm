# carve-wasm

WebAssembly build of the [Carve](https://markup-carve.github.io/carve/) markup
renderer, compiled from
[carve-rs](https://github.com/markup-carve/carve-rs). It runs the Rust
implementation in browsers and powers the **Rust (WASM)** engine in the
[Carve playground](https://markup-carve.github.io/carve/playground).

## Install

```bash
npm install @markup-carve/carve-wasm
```

> Publishing to npm is pending. Until the package is live, build it locally
> using the [build guide](docs/building.md) or consume the generated `pkg/`
> directory directly.

## Render Carve

The published package uses the bundler target for webpack, Vite, Rollup, and
similar tools. WASM initialization is automatic, so its exports are
synchronous.

```js
import { toHtml, toHtmlFull, toHtmlWithOptions } from '@markup-carve/carve-wasm'

toHtml('# Hello, Carve!')
toHtmlFull('# Hello\n\n``` mermaid\ngraph TD; A-->B\n```\n')
toHtmlWithOptions(source, {
  extensions: ['glossary', 'table-of-contents'],
  rawHtml: false,
  sections: true,
})
```

`toHtml` uses the core renderer. `toHtmlFull` enables the extension set used
by the playground preview. `toHtmlWithOptions` lets the caller choose
extensions and rendering policy. Unknown extension names throw instead of
silently producing incomplete output.

Other core targets include `toMarkdown`, `toPlainText`, `toAnsi`, and
`toCarve`. Each has a `*WithOptions` form.

## Convert existing content

The package imports HTML, Markdown, Djot, and BBCode. `fromHtml`,
`fromMarkdown`, `migrateDjot`, and `migrateBbcode` return the converted value
with a fidelity report. The simpler `fromDjot` and `fromBbcode` helpers return
only the converted string.

Markdown, Djot, and BBCode reports currently mark fidelity as unverified. An
empty diagnostic list does not claim that every construct was preserved.

```js
import { fromHtml, fromMarkdown, htmlToAst, htmlToCarve } from '@markup-carve/carve-wasm'

const { value, report } = htmlToCarve('<p>Hello <strong>world</strong></p>', 'safe')
const { value: ast } = htmlToAst('<p>Hello <b>world</b></p>', 'safe')

fromHtml(html, 'semantic')
fromMarkdown(markdown)
```

HTML modes are `safe` (the default), `semantic`, and trusted-only
`roundtrip`. `htmlToAst` returns the serialized Carve AST without passing it
through the source writer, so its report can differ from `htmlToCarve` when a
construct is representable in the tree but not in Carve source.

## Common host APIs

- `extensions()` returns the extension names supported by the bundled engine.
- `toCarvePatch()` and `applySourcePatch()` create and apply stale-safe UTF-8
  byte edits.
- `toHtmlWithSymbols()` resolves configured `:name:` symbols. Symbol values
  are trusted raw output and must not come from user input.
- `parseJson()`, `astJsonToHtml()`, and the other AST functions use the PART 12
  AST JSON shared by Carve implementations.
- Include expansion calls a synchronous host resolver, records dependencies,
  and enforces depth, byte, and call budgets. The resolver decides which paths
  to refuse.
- Incremental parsing, tree patches, three-way merge, and ProseMirror helpers
  support browser editors.

The [complete API and usage reference](docs/reference.md) documents these
APIs, renderer profiles, resource limits, includes, AST fields, and the
TypeScript declarations.

## Security

Use `rawHtml: false` with a restrictive profile such as `comment` for
untrusted documents. Symbols and static renderer callbacks are trusted
configuration and may emit unescaped HTML. Treat source patches as trusted
edit instructions, not as signed data. A profile rejection throws.

Carve also enforces URL-scheme and attribute hardening in the renderer. See
the language [security documentation](https://markup-carve.github.io/carve/security)
for the complete model.

## Development

Contributor documentation lives under `docs/`:

- [Building and testing](docs/building.md)
- [The carve-rs engine pin](docs/engine-pin.md)
- [Release process](docs/releasing.md)
