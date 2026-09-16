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

### HTML migration

`htmlToCarve(html, mode)` returns `{ value, report }`, using the same HTML5
import policy and canonical writer as carve-rs. Modes are `safe` (default),
`semantic`, and trusted-only `roundtrip`.

```js
const { value, report } = htmlToCarve('<p>Hello <strong>world</strong></p>', 'safe')
```

Portable migration code can use `fromHtml(html, mode)` and
`fromMarkdown(markdown)`. Both return `{ value, report }`. Version 2 reports use
the shared fidelity vocabulary. Markdown, Djot, and BBCode conservatively emit
`fidelity-unverified` as `dropped` / `fallback` until their importers expose
construct-level outcomes; an empty diagnostic list is therefore never used to
imply fidelity that was not assessed.

### Core renderer

Renders Carve markup to HTML with no extensions enabled.

The published package is the **bundler** target (webpack, Vite, Rollup, ...):
the wasm initializes automatically, so the exports are synchronous - no `init()`
call.

```js
import { toHtml } from '@markup-carve/carve-wasm'

// The other core targets are toMarkdown, toPlainText, toAnsi, and toCarve,
// each with a *WithOptions form that takes the options object below.

const html = toHtml('# Hello, Carve!')
document.body.innerHTML = html
```

### Source-preserving patches

Tools can prepare canonical formatting as stale-safe UTF-8 byte edits:

```js
const patch = toCarvePatch(source)
const formatted = applySourcePatch(source, patch)
```

`createSourcePatch(source, replacement, kind, code)` prepares the same wire
shape for another complete replacement. Its ranges are UTF-8 byte offsets. The
fingerprint and byte length catch accidental staleness; they are not a
cryptographic signature. Treat a patch as trusted edit instructions.

### Extensions

`extensions()` reports every extension this build accepts. The list comes from
the engine, so it cannot fall behind what the engine has:

```js
import { extensions, toHtmlWithOptions } from '@markup-carve/carve-wasm'

extensions()
// ['autolink', 'citations', 'code-callouts', 'code-group', ...]

toHtmlWithOptions(src, { extensions: ['glossary', 'table-of-contents'] })
```

Names are kebab-case; snake_case (`math_block`) is accepted too. An unknown name
throws, because an ignored extension renders as missing behavior that looks
like a Carve bug.

### Full renderer (preview set)

`toHtmlFull` enables the preview set the playground uses: tab normalisation,
`<details>` fences, Mermaid diagrams, wikilinks, autolink, list-table, math
blocks, heading permalinks, citations, code callouts, external-link decoration,
code groups, and tabs.

It is a curated subset rather than everything registered. Extensions that
rewrite a document that never asked - `heading-numbers` numbers every heading,
`table-of-contents` injects a TOC - are wrong for a preview. Name them
explicitly through `toHtmlWithOptions` when you want them.

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

### Section wrappers

A top-level heading is wrapped, along with the content following it up to the
next same-or-shallower heading, in a `<section>` carrying the heading's id (spec
PART 9 §13). Only the id moves - `{#install .featured}` gives
`<section id="install"><h2 class="featured">` - and a heading inside a
blockquote, div or list item is not wrapped at all.

`toHtmlWithOptions` is the general entry point, and `sections: false` renders
headings flat with the id back on the `<h*>`:

```js
import { toHtmlWithOptions } from '@markup-carve/carve-wasm'

toHtmlWithOptions('# A\n\np\n', { sections: false })
// '<h1 id="A">A</h1>\n<p>p</p>'

toHtmlWithOptions(src, { sections: false, symbols: { rocket: '🚀' }, full: true })
```

Every field is optional. Omitting the object, or passing `null`, renders with
defaults, so the three shorthands above remain the zero-config forms.

| Field | Default | What it does |
|---|---|---|
| `sections` | `true` | Wrap each top-level heading and its content in a `<section>` |
| `symbols` | none | `:name:` to value, TRUSTED-RAW (see above) |
| `extensions` | none | Array of registry names; takes precedence over `full` |
| `full` | `false` | Enable the preview extension set |
| `rawHtml` | `true` | Emit an explicit passthrough as markup; `false` escapes it |
| `profile` | none | `full` / `article` / `comment` / `minimal`; rejection throws |
| `profileBaseHost` | none | The host the profile's link policy counts as internal |
| `mode` | `interactive` | `static` renders the self-contained form: no client scripts |
| `sourceLine` | `false` | Stamp top-level blocks with `data-source-line` |
| `positions` | `false` | Keep source offsets on the nodes |
| `labels` | none | Override engine-written strings, for a page not in English |
| `smartTypography` | `glyph` | `source` keeps the author's run instead of the glyph |
| `lowercaseHeadingIds` | `false` | Lowercase the generated heading ids |
| `asciiHeadingIds` | `off` | `fold` transliterates, `strict` guarantees ASCII |
| `mentionUrl` | none | URL template for `@mention`; `{name}` / `{user}` take the encoded name |
| `tagUrl` | none | URL template for `#tag`; `{name}` takes the encoded name |

An unrecognized key is ignored, because the object is configuration and a typo
should not break a render. A recognized key with the wrong type throws a
`TypeError` instead of being coerced: JS truthiness would read
`{ sections: 'false' }` as `true`, the opposite of what was written.

`renderers` is the one recognized key `toHtmlWithOptions` refuses; it belongs to
[`toHtmlWithRenderers`](#static-renderers).

This exists for a host whose CSS or JS assumes rendered blocks are direct
children of the content container - the `.stack > * + *` spacing idiom,
`:first-child`, `nth-child()` counting, `element.children` walks - all of which
stop matching once a wrapper sits in between. It is the one output change that
breaks a document whose *source* migrated cleanly.

Nothing else changes: ids, collision dedup, `</#id>` cross-references, implicit
`[Heading][]` references and heading numbering all resolve against the slug
rather than the element carrying it. The endnotes
`<section role="doc-endnotes">` is a separate construct and is still emitted.

### Rendering a document you did not write

`rawHtml: false` renders an explicit passthrough - the `=html` raw block and the
`` `…`{=html} `` inline raw span - as escaped text instead of markup. It is the
switch carve-js spells `allowRawHtml`.

```js
toHtmlWithOptions(fromTheReader, { rawHtml: false })
```

Reach for it whenever the document comes from somewhere other than the person
running the page: a shared link, a comment field, a pasted file. A passthrough is
the one construct that puts author-controlled markup on your origin, so leaving
it on for a document a reader supplied is a way to run their script.

The symbols map is unaffected and stays TRUSTED-RAW either way - it is
configuration the host wrote, not content the document carries.

### Rendering with a profile

`rawHtml: false` closes the passthrough vector. It does not cap input length,
deny a construct, or constrain link schemes - that is what a profile is for, and
the four presets match the ones the spec and the sibling bindings describe.

```js
toHtmlWithOptions(fromTheReader, { profile: 'comment', rawHtml: false })
```

**A rejected document throws.** The engine's infallible entry point turns a
profile rejection - input past `max_length`, or a denied construct when the
action is Error - into an EMPTY STRING, and a caller cannot tell that from a
document that legitimately rendered to nothing. This binding renders through the
fallible one, so a rejection arrives as an `Error` whose `name` is
`ProfileViolationError` and whose `violations` array carries one message per
refused construct.

```js
try {
  html = toHtmlWithOptions(fromTheReader, { profile: 'comment' })
} catch (error) {
  if (error.name === 'ProfileViolationError') report(error.violations)
}
```

**Every target takes the profile, not only HTML.** `toMarkdownWithOptions`,
`toPlainTextWithOptions`, `toAnsiWithOptions`, `toCarveWithOptions` and
`parseJsonWithOptions` read the same options object, so a document held to a
profile on its way to HTML is held to it on its way to Markdown or into a stored
tree. Without them a host could export the same untrusted document unfiltered.

```js
toMarkdownWithOptions(fromTheReader, { profile: 'comment' })
```

What those targets actually read is narrower than HTML's list: `profile` and
`smartTypography` change their output, extensions run, and `symbols`, `labels`,
`sections`, `sourceLine`, `mode` and the heading-id switches are HTML-side
concerns the engine's other renderers do not consult. `renderers` is refused
there as it is on `toHtmlWithOptions`.

`toCarveWithOptions` is narrower again and reads `profile` alone. The canonical
writer is parse-only by contract, so extensions and `smartTypography` are inert
there; they are accepted rather than refused so that one options object can be
handed to every target.

`applyProfile` runs the same filter over an AST-JSON document and hands back the
filtered tree instead of HTML, for a host that wants to store, diff or re-render
what the filter left. `violations` reports what it degraded or stripped, which
the HTML path discards.

```js
const { json, violations } = applyProfile(parseJson(fromTheReader), 'comment')
if (violations.length > 0) tell(violations.map((v) => v.message))
const html = astJsonToHtml(json)
```

It reads `profileBaseHost` and `smartTypography` from its options object and
nothing else. It does **not** enforce the profile's `max_length`, which the
engine applies to the SOURCE bytes before a parse - a host filtering untrusted
input still needs that bound on the way in.

### Static renderers

`mode: 'static'` renders the self-contained form, which carries no client
scripts - so a formula in the document has to be typeset while the HTML is being
written, by the host. `toHtmlWithRenderers(source, options)` takes the same
options object plus a `renderers.math` callback and returns
`{ html, rendererErrors }`.

```js
import katex from 'katex'
import { toHtmlWithRenderers } from '@markup-carve/carve-wasm'

const { html, rendererErrors } = toHtmlWithRenderers(source, {
  mode: 'static',
  extensions: ['math-block'],
  renderers: {
    math: (tex, display) => katex.renderToString(tex, { displayMode: display }),
  },
})
```

The callback is called once per ` ```math ` fence, with the TeX source and a
display flag. Without it, static output keeps the `\[…\]` source for a client to
typeset later - never blank.

> **Security: what a renderer returns is TRUSTED RAW HTML.**
> The string is inserted **unescaped**, the same trust class as a `symbols`
> value. There is one difference worth stating: a symbol value is host
> configuration keyed by a **name**, while a renderer is host configuration that
> is **handed document content** and typically echoes some of it back. A host
> rendering documents it did not author is accepting whatever its renderer makes
> of that input, so the escaping is the renderer's job.

**The callback must be synchronous.** wasm-bindgen cannot await across it, so an
`async` renderer returns a Promise the engine has no way to resolve. That is
reported as a failure rather than stringified into the document.

It is also why there is no `renderers.diagrams` yet: Mermaid's `render` returns
a Promise from v10 on, so it cannot be passed to a synchronous callback at all,
and a host could only supply a lookup into diagrams it rendered beforehand.
Passing `diagrams` throws, rather than being ignored - an ignored key renders the
fence as source with nothing to say the configuration did nothing.

A callback that throws, or returns anything but a string, does not abort the
render. The node it was called for emits nothing and the failure is reported:

```js
const { html, rendererErrors } = toHtmlWithRenderers(source, {
  mode: 'static',
  extensions: ['math-block'],
  renderers: { math: () => { throw new Error('bad TeX') } },
})
// rendererErrors: [{ renderer: 'math', display: true, source: 'E = mc^2', message: '…bad TeX' }]
```

That is why this is a separate entry point: `toHtmlWithOptions` returns a bare
string with nowhere to report a failing callback, and a silently empty figure is
the degradation `lintCarve` exists to warn about. Every other check is at READ
time, matching the rest of the options object - a non-callable `math`, a
non-object `renderers`, and `diagrams` all throw a `TypeError` before the render
starts, so a misconfigured host finds out without needing a document that
happens to contain a formula.

### Editing a tree, and reading one back

`parseJson` serializes a document out. `astJsonToHtml` renders one back, and
`astJsonToCarve` writes one back as source, so a host that reads the tree in
order to change something can display and save the result without a server.

```js
const tree = JSON.parse(parseJson(source))
tree.children.unshift({ type: 'heading', level: 1, children: [{ type: 'text', value: 'Added' }] })
const html = astJsonToHtml(JSON.stringify(tree), { full: true })
const carve = astJsonToCarve(JSON.stringify(tree))
```

A tree carrying something no Carve source can spell is refused by
`astJsonToCarve` rather than written approximately, and an invalid tree throws
from either.

`lintCarve` returns the degradation diagnostics as
`{ line, column, rule, message, start, end }`, with the rule ids carve-js and
carve-php use for the same triggers. Offsets are BYTE offsets into the source,
matching the engine.

### ProseMirror

`toProseMirror(source)` converts Carve to the ProseMirror document shape, and
`fromProseMirror(doc)` writes one back as canonical Carve source. ProseMirror
runs in a browser and nowhere else, so this binding is the whole audience for
the engine's bridge - without it a host wiring a Carve editor either round-trips
to a server or reimplements the node mapping in JS, where it drifts from the
engine's.

```js
const { json, dropped, degraded } = toProseMirror(source)
editor.commands.setContent(JSON.parse(json))
const saved = fromProseMirror(JSON.stringify(editor.getJSON()))
```

`json` is a JSON string, the same choice `parseJson` makes. The two maps are
`Carve node type -> reason`: `dropped` where the content is gone (an
abbreviation definition has no editor node), `degraded` where the text survives
without its node type (a soft break becomes whitespace, smart typography
resolves to the glyph). Both are empty for a document the model holds exactly.

A payload the schema map does not describe is refused rather than written
approximately. Round-tripping normalizes the source the way `toCarve` does, and
the reported degradations do not come back - `a ... b` returns as `a … b`.

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
| `toHtmlWithOptions` | `(source: string, options?: object \| null) => string` | General form; see the options table above. Throws `ProfileViolationError` when a profile rejects the document |
| `toHtmlWithRenderers` | `(source: string, options?: object \| null) => StaticRenderResult` | The options object plus `renderers.math`, for `mode: 'static'`; returns `{ html, rendererErrors }` |
| `toMarkdownWithOptions` | `(source: string, options?: object \| null) => string` | Markdown under the same options object. Throws `ProfileViolationError` when a profile rejects the document |
| `toPlainTextWithOptions` | `(source: string, options?: object \| null) => string` | Plain text under the same options object |
| `toAnsiWithOptions` | `(source: string, options?: object \| null) => string` | ANSI text under the same options object |
| `toCarveWithOptions` | `(source: string, options?: object \| null) => string` | Canonical Carve under the same options object; reads `profile` only |
| `parseJsonWithOptions` | `(source: string, options?: object \| null) => string` | The AST as JSON under the same options object; positions are always on |
| `toHtmlWithReport` | `(source: string, strict?: boolean, maximum?: number) => RenderResult` | HTML plus bounded `raw-format-dropped` losses; strict mode throws `RenderLossError` |
| `toMarkdownWithReport` | `(source: string, strict?: boolean, maximum?: number) => RenderResult` | Checked Markdown render |
| `toPlainTextWithReport` | `(source: string, strict?: boolean, maximum?: number) => RenderResult` | Checked plain-text render |
| `toAnsiWithReport` | `(source: string, strict?: boolean, maximum?: number) => RenderResult` | Checked ANSI render |
| `toCarveWithReport` | `(source: string, strict?: boolean, maximum?: number) => RenderResult` | Checked canonical Carve render (lossless) |
| `parseJson` | `(source: string) => string` | The parsed AST as JSON (PART 12 exchange shape) |
| `astJsonToHtml` | `(json: string, options?: object \| null) => string` | Render an AST-JSON document; takes the same options object |
| `astJsonToCarve` | `(json: string) => string` | Write an AST-JSON document back as canonical Carve source |
| `applyProfile` | `(json: string, profile: string, options?: object \| null) => ProfileFilterResult` | Filter an AST-JSON document through a profile, keeping the tree and what the filter did |
| `lintCarve` | `(source: string) => LintWarning[]` | Degradation diagnostics, with the rule ids carve-js and carve-php share |
| `lintCarveWithOptions` | `(source: string, options?: object \| null) => LintWarning[]` | The same linter for the extension set the host renders with |
| `lintAccessibility` | `(source: string) => AccessibilityDiagnostic[]` | The second diagnostic family, with its own rule ids and a severity |
| `stampCarve` | `(formatted: string, generatedBy: string, form?: "line" \| "block") => string` | Write the provenance marker `readStamp` reads |
| `sanitizeSvg` | `(source: string, options?: object \| null) => SanitizeResult` | Sanitize an SVG document; strict unless an option says otherwise |
| `parseLocator` | `(loc: string) => ParsedLocator` | Parse a citation locator into label, value and suffix |
| `parseSourceLayoutJson` | `(source: string) => string` | The PART 12 §13 source-layout sidecar |
| `markdownToAstJson` | `(source: string) => string` | Import Markdown straight to the tree, skipping the Carve-source round trip |
| `toProseMirror` | `(source: string) => ProseMirrorResult` | Convert to the ProseMirror document shape, with what the model could not hold |
| `fromProseMirror` | `(doc: string) => string` | Convert a ProseMirror document back to canonical Carve source |
| `readStamp` | `(source: string) => { version, generatedBy } \| null` | The document's provenance marker, if it carries one |
| `needsReview` | `(source: string, currentVersion: string) => boolean` | Whether the stamp predates `currentVersion`; unstamped counts as yes |
| `fromDjot` | `(source: string) => string` | Convert Djot source to Carve |
| `migrateDjot` | `(source: string) => { value, report }` | Convert Djot with a v2 fidelity report |
| `fromBbcode` | `(source: string) => string` | Convert BBCode source to Carve; throws past the engine's size cap |
| `migrateBbcode` | `(source: string) => { value, report }` | Convert BBCode with a v2 fidelity report; throws past the engine's size cap |
| `createSourcePatch` | `(source, replacement, kind, code) => SourcePatch` | Build a minimal UTF-8 byte-range patch |
| `toCarvePatch` | `(source: string) => SourcePatch` | Preview canonical formatting as a patch |
| `applySourcePatch` | `(source: string, patch: SourcePatch) => string` | Validate and apply trusted patch instructions |
| `version` | `() => string` | Returns the carve-wasm package version |

### The parsed AST

`parseJson` returns the document as a JSON string - the [PART 12 exchange
shape](https://markup-carve.github.io/carve/ast-json), the same tree every Carve
engine publishes, so a consumer written against one implementation reads
another's output.

```js
import { parseJson } from '@markup-carve/carve-wasm'

const ast = JSON.parse(parseJson('# Title\n\nBody[^a].\n\n[^a]: note\n'))
ast.children.map((n) => n.type) // ['heading', 'paragraph', 'footnote']
ast.children[0].pos             // { startLine: 1, startColumn: 1, ... }
```

The root carries exactly `type`, `children` and `srcByteLength`; frontmatter and
footnote definitions are block nodes inside `children`, not root fields. Every
node except the root carries `pos` when the engine could place it - 1-based
lines and columns, 0-based offsets, ends exclusive, counted in Unicode
**codepoints**, not bytes or UTF-16 units. A node the engine could not place,
such as reassembled table-cell text, carries no `pos` at all rather than an
invented one.

A string rather than a JS object: the caller runs `JSON.parse`, which the
browser does natively and faster than building the object graph across the wasm
boundary one property at a time - and it keeps the bytes available for a caller
that stores or forwards them.

## Development

The reference material a contributor needs lives in `docs/`, so this page can
stay about USING the package:

- [Building](docs/building.md) - the wasm-pack targets, pointing the tests at a
  built package, and the rendering-only browser profile.
- [The carve-rs pin](docs/engine-pin.md) - why the engine is pinned to an exact
  commit, and how to move it.
- [Releasing](docs/releasing.md) - the gate, what it refuses, and the runbook
  for cutting a version.
