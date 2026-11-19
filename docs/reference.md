# Carve 0.2 WASM API

The binding uses the Rust engine's JSON boundary. Calls use 0.2 names, options and error records. A thrown convenience-call error carries `name`, `category`, `status`, and `message`; `call` returns the equivalent JSON response instead.

| Call | Return |
| --- | --- |
| `call(requestJson)` | JSON response text, with `ok` or `error` |
| `parse(source, options?)` | 0.2 tree |
| `parseWithDiagnostics(source, options?)` | tree, diagnostics, suppressed count |
| `render(source, options?)` | HTML string |
| `renderWithDiagnostics(source, options?)` | HTML, diagnostics, suppressed count |
| `lint(source, options?)` | diagnostic entries |
| `renderTree(tree, options?)` | HTML string |
| `encodeTree(tree)`, `decodeTree(json)` | JSON text or a 0.2 tree |
| `includeDependencies(tree, file?)` | dependency entries |
| `write(tree, target, options?)` | registered writer output |
| `format(source, options?)` | canonical source, when its writer is registered |

`toHtml` and `toHtmlWithOptions` are aliases for `render`. `toMarkdown`, `toPlainText`, `toAnsi`, and `toCarve` retain their entry points. Their writers are not registered yet and they throw a clear availability error. `importDocument` and `extensions` similarly report their pending Rust APIs.

`write` accepts `html`, `carve`, `markdown`, `plain`, or `ansi` as a target. HTML is currently registered. A target's output can depend on options such as `roundtrip`, `profile`, and `strictLosses`; the Rust engine validates which options each call reads.

## Rendering policy

`rawHtml` defaults to false. The engine's `urlPolicy` default denies unsafe schemes. To allow raw HTML for trusted source only:

```js
render(source, { rawHtml: true })
```

The 0.2 options are `headingIds`, `mode`, `profile`, `rawHtml`, `urlPolicy`, `sections`, `smartTypography`, `sourceLines`, `strictLosses`, `roundtrip`, `symbols`, `labels`, `social`, `includes`, `extensions`, and `renderers`, when the operation reads them. `alignTables` and `normalize` belong to formatting and writing. Unknown, invalid, and unused options are refused. The JSON boundary currently accepts only empty `extensions` and `renderers` values because it cannot carry host code.

The JSON `includes` option takes a file-system root in the Rust engine. That resolver is unavailable in a browser. For browser includes, pass a resolved tree once the extension host supports a resolver callback. The raw JSON boundary remains useful for `includeDependencies` on a tree.

## Tree and patch data

Tree JSON uses the Carve 0.2 schema. An old 0.1 tree must be migrated before `decodeTree`. `createSourcePatch` and `applySourcePatch` are local UTF-8 byte-range helpers, not engine tree patch calls. The 0.2 API has no incremental session, AST patch, AST merge, ProseMirror, trace, or stamp calls.
