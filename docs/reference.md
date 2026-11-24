# Carve 0.2 WASM API

The binding uses the Rust engine's JSON boundary. Calls use 0.2 names, options and error records. A thrown convenience-call error carries `name`, `category`, `status`, and `message`; `call` returns the equivalent JSON response instead.

| Call | Return |
| --- | --- |
| `call(requestJson)` | JSON response text, with `ok` or `error` |
| `parse(source, options?)` | 0.2 tree |
| `parseWithDiagnostics(source, options?)` | tree, diagnostics, suppressed count |
| `parseWithLayout(source, options?)` | tree, layout sidecar and record |
| `render(source, options?)` | HTML string |
| `renderWithDiagnostics(source, options?)` | HTML, diagnostics, suppressed count |
| `lint(source, options?)` | diagnostic entries |
| `renderTree(tree, options?)` | HTML string |
| `encodeTree(tree)`, `decodeTree(json)` | JSON text or a 0.2 tree |
| `migrateTree(oldTree)` | 2.0 tree and record |
| `includeDependencies(tree, file?)` | dependency entries |
| `write(tree, target, options?)` | registered writer output |
| `writeWithDiagnostics(tree, target, options?)` | writer output and diagnostic record |
| `format(source, options?)` | canonical source |
| `formatWithDiagnostics(source, options?)` | canonical source and diagnostic record |

`toHtml` and `toHtmlWithOptions` are aliases for `render`. `toMarkdown`, `toPlainText`, `toAnsi`, and `toCarve` use the registered writers, including their `WithOptions` forms. `toCarvePatch`, `importDocument` and `extensions` are also available.

`write` accepts `html`, `carve`, `markdown`, `plain`, or `ansi` as a target. The writer calls read format options such as `alignTables` and `normalize`; render policy options such as `rawHtml` belong to `render` and `renderWithDiagnostics`. The Rust engine refuses options that a call does not read.

## Rendering policy

`rawHtml` defaults to false. The engine's `urlPolicy` default denies unsafe schemes. To allow raw HTML for trusted source only:

```js
render(source, { rawHtml: true })
```

The 0.2 options are `headingIds`, `mode`, `profile`, `rawHtml`, `urlPolicy`, `sections`, `smartTypography`, `sourceLines`, `strictLosses`, `roundtrip`, `symbols`, `labels`, `social`, `headingShift`, `includes`, `extensions`, and `renderers`, when the operation reads them. `alignTables` and `normalize` belong to formatting and writing. Unknown, invalid, and unused options are refused. `extensions` takes the standard extension names that `extensions()` lists, and `renderers` maps one of them to `"standard"`; host extension code has no form on the JSON boundary (C2-T-010).

## Includes

The package is built without file-system access, so the host supplies included files. `includes` on `parse`, `parseWithDiagnostics`, `parseWithLayout`, `render`, `renderWithDiagnostics`, `lint`, `format` and `formatWithDiagnostics` takes a synchronous resolver, alone or as `{ resolver, file?, depth?, bytes? }`, the same shape as carve-js's `IncludeConfig`:

```js
const files = { 'part.crv': 'Included *text*.' }
const resolver = ({ path }) =>
  path in files ? { text: files[path], id: path } : { error: 'not_found' }

render('{{ part.crv }}', { includes: { resolver, file: 'main.crv' } })
// <p>Included <strong>text</strong>.</p>
```

The resolver receives `{ path, section?, options, from? }` for each directive and returns `{ text, id }`, where `text` is a string or a `Uint8Array` of UTF-8, or `{ error }` with `not_found`, `outside_root`, `unreadable` or `non_text`. A resolver that throws or returns another shape counts as `unreadable`. The engine applies its own include rules: the cycle check, the depth and byte limits (`depth` and `bytes` only lower them), region and section selection, and include diagnostics with the included file. `file` is the id of the top-level document, which a cycle can return to. `call(requestJson)` takes no resolver, since JSON text has none; there `includes` is `{"root": DIR}`, which this build refuses. `includeDependencies(tree)` lists the include directives of a tree, so a host can fetch the files before it parses.

## Tree and patch data

Tree JSON uses the Carve 0.2 schema. An old 0.1 tree must pass through `migrateTree` before `decodeTree`; `parseWithLayout` returns the source layout sidecar. `createSourcePatch` and `applySourcePatch` are local UTF-8 byte-range helpers, not engine tree patch calls. The 0.2 API has no incremental session, AST patch, AST merge, ProseMirror, trace, or stamp calls.
