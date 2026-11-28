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
| `toHtmlWithRenderers(source, options?)` | HTML, diagnostics, suppressed count, with host renderers and transforms |
| `selectIncludeRegion(source, name, mode)` | `{ start, end }` content lines of a region |
| `parseSnapshot(source)`, `reparse(source, changesJson)` | JSON text: source, tree, layout and changed ranges |
| `new ParserSession(source)` | `snapshot()` and `edit(changesJson)`, the same JSON text |

`toHtml` and `toHtmlWithOptions` are aliases for `render`. `toMarkdown`, `toPlainText`, `toAnsi`, and `toCarve` use the registered writers, including their `WithOptions` forms. `toCarvePatch`, `importDocument` and `extensions` are also available. `importDocument(source, from)` takes `from` as `html`, `markdown`, `gfm` or `djot`.

`write` accepts `html`, `carve`, `markdown`, `plain`, or `ansi` as a target. The writer calls read format options such as `alignTables` and `normalize`; render policy options such as `rawHtml` belong to `render` and `renderWithDiagnostics`. The Rust engine refuses options that a call does not read.

## Rendering policy

`rawHtml` defaults to false. The engine's `urlPolicy` default denies unsafe schemes. To allow raw HTML for trusted source only:

```js
render(source, { rawHtml: true })
```

The 0.2 options are `headingIds`, `mode`, `profile`, `rawHtml`, `urlPolicy`, `sections`, `smartTypography`, `sourceLines`, `strictLosses`, `roundtrip`, `symbols`, `labels`, `social`, `headingShift`, `includes`, `extensions`, and `renderers`, when the operation reads them. `alignTables` and `normalize` belong to formatting and writing. Unknown, invalid, and unused options are refused. `extensions` takes the standard extension names that `extensions()` lists, and `renderers` maps one of them to `"standard"`; host extension code has no form on the JSON boundary (C2-T-010).

## Host renderers and transforms

Host code has no form in JSON (C2-T-010), so it is an option of the library calls. `toHtmlWithRenderers` builds an engine host with it and returns `{ html, diagnostics, suppressedDiagnostics }`. `transforms` is a list of functions that each take the resolved tree and return a tree. `renderers` maps a host extension name to a function of the extension node, used in both channels, or to `{ interactive?, static?, version? }`; a standard extension still takes only `"standard"` or `{ standard: { mode } }`. A transform claims the nodes a host renderer renders:

```js
toHtmlWithRenderers(source, {
  mode: 'static',
  transforms: [tree => claimMermaidFences(tree)],
  renderers: { 'ex.mermaid': { static: node => prerendered.get(node.fallback[0].value) } },
})
```

The returned string is inserted as trusted HTML. A callback must be synchronous. A renderer that throws or returns no string leaves the fallback and reports `extension-renderer-failed`; a failed transform keeps the previous tree and reports `extension-transform-failed`. The call reads the render options and an `includes` resolver as `render` does.

## Include regions and editor sessions

`selectIncludeRegion(source, name, mode)` returns the 1-based inclusive content lines `{ start, end }` of `@region:name` with the engine's marker rules; `mode` is `"carve"` or `"code"`, which also reads the source-comment markers. An empty region has `end === start - 1`. A missing region or a malformed or unmatched marker throws an `IncludeRegionError` whose `rule` is `include-region-invalid`.

`parseSnapshot`, `reparse` and `ParserSession` keep the parse state an editor reuses. Changes are a JSON array of `{ range: [start, end], replacement }` in UTF-8 byte offsets; a malformed change throws a `TypeError` and leaves a session unchanged. Each result is JSON text with `source`, `document` (the tree `parse` returns), `sourceLayout`, `changedSource`, `reusedPreviousTree` and `parsedSourceBytes`. Call `free()` on a session when done.

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

## ProseMirror / Tiptap bridge

The engine's ProseMirror bridge (`carve::api::prosemirror`) writes and reads the schema of the carve-grammars Tiptap kit, so a document the kit saves loads here and the other way round. It is in the full package (feature `prosemirror`) and not in the rendering-only build.

```js
const { doc, blanked } = toProseMirror(source)   // or a 0.2 tree; editor.commands.setContent(doc)
const saved = proseMirrorToCarve(editor.getJSON()) // canonical Carve source
const tree = fromProseMirror(editor.getJSON())     // the 0.2 tree
```

- `toProseMirror(sourceOrTree, options?)` returns `{ doc, json, blanked }`. A node the editor content model cannot hold becomes a `carveBlock` or `carveInline` atom that carries it whole. A link or image destination the URL policy denies is `null` in the editor attribute and kept in `carve`; `blanked` lists those destinations.
- `fromProseMirror(doc, options?)` returns the tree, and `proseMirrorToCarve(doc, options?)` the source. `doc` is an object or JSON text. A node or mark with no Carve node throws a `ProseMirrorError`; a tree the reader refuses throws a `CarveTreeError`.
- `fromProseMirrorWithReport(doc, options?)` returns `{ tree, origins, pointers, ignored }`: per top-level block the index of the loaded block it came from (`null` for a block the editor added), the JSON pointer of the editor node for each tree path (to place a lint finding), and each attribute the save did not read, such as a `textAlign`.
- `options`: `urlPolicy` (`{ deny: [...] }` or `{ allow: [...] }`, as for rendering) and `mapping`, the application's own nodes and marks: `{ nodes: { placeholderToken: { as: 'span', class: 'placeholder', attributes: ['name'] } }, marks: { ... } }`. A mapped node saves as a span or div of that class with its attributes as key/value pairs, and loads back as the host node.

## Tree and patch data

Tree JSON uses the Carve 0.2 schema. An old 0.1 tree must pass through `migrateTree` before `decodeTree`; `parseWithLayout` returns the source layout sidecar. `createSourcePatch` and `applySourcePatch` are local UTF-8 byte-range helpers, not engine tree patch calls. The 0.2 API has no AST patch, AST merge, trace, or stamp calls.
