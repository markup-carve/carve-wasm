# Changelog

Notable changes to `@markup-carve/carve-wasm`.

The parser and renderer are carve-rs, compiled to WebAssembly and pinned to a
revision or version in `Cargo.toml`, so an engine bump can change rendering without a
line of Rust in this repository changing. Engine bumps therefore get an entry of
their own.

## [Unreleased]

## [0.1.6] - 2026-10-05

### Breaking

- Edit offsets, include budgets and checked-render limits reject values outside
  `0..4294967295`, fractions and nonfinite values. Checked renderers reject
  nonboolean `strict` values instead of coercing them.
- ProseMirror conversion reports canonical source changes under
  `degraded.document` when no node-level loss was reported.

- `toHtmlWithReport` reports one `destination-denied` loss for each link,
  autolink or image whose URL scheme the sink denylist blanks. The message names the sink: "Blanked a denied destination scheme" for
  a link or autolink, "Blanked a denied image source" for an image. The HTML is
  unchanged (`href=""` and `src=""` stay as they were), but a `strict` render of
  such a document now throws `RenderLossError`, and a consumer that switches on
  `code` needs a third case beside `raw-format-dropped` and `ruby-flattened`. A
  refused `strict` render's message now reads "render would lose N nodes"
  instead of "render would drop N raw nodes", so code that matches on the
  message needs updating (#151, #156, markup-carve/carve-rs#2243,
  markup-carve/carve-rs#2246, markup-carve/carve#2679, markup-carve/carve#2681).

### Fixes

- HTML round trips settle for all imported corpus examples. Empty definition
  terms are preserved as raw HTML in trusted roundtrip mode or dropped and
  reported in safe imports while keeping definition order.
- Edit offsets are checked before narrowing to WASM's 32-bit size, so an offset
  above `4294967295` cannot wrap around and edit another part of the document.
- Published declarations include `IncludeExpansion`, include options, rendering
  reports and typed renderer options. AST positions are documented as Unicode
  codepoints; edits and source patches use UTF-8 bytes.

- `toCarve` no longer crashes on emphasis inside a link label, keeps the
  parentheses and backslashes of a denied URL scheme so formatting no longer
  changes the destination, and doubles a backslash in a quoted attribute, class
  or title only where the re-parse needs it (markup-carve/carve-rs#2229,
  markup-carve/carve-rs#2248, markup-carve/carve-rs#2224).
- HTML import stops pairing a superscript or subscript opener with a
  reference-shaped tail's bracket, Djot import keeps emphasis, attributes, list
  markers and quotes it previously altered, and the Markdown, plain text and
  ANSI targets keep a list table's grouping label (markup-carve/carve-rs#2225,
  markup-carve/carve-rs#2235, markup-carve/carve-rs#2236).
- An overindented quote marker after a fence on a list item's marker line stays
  item text, a lazy line inside a quoted comment keeps its indentation, and
  link, image and heading references resolve inside line blocks
  (markup-carve/carve-rs#2228, markup-carve/carve-rs#2231,
  markup-carve/carve-rs#2249).
- `lintCarve` reports list over-indentation once per block, and no longer warns
  about an empty footnote or definition body or about list padding before a
  quote (markup-carve/carve-rs#2226, markup-carve/carve-rs#2234).
- `parseJson` reports reference and footnote definition positions in the
  original input, so a CRLF line ending or a leading BOM no longer shifts them
  (markup-carve/carve-rs#2239).
- Every render target enforces the depth ceiling in citation definitions,
  extension summaries and captions (markup-carve/carve-rs#2242).
- Tables keep multiple bodies, intermediate body headers, empty bodies and
  per-body row-header counts through `parseJson`, `toCarve` and `toHtml`, and
  decimal column widths keep their precision. HTML import keeps every empty
  `<tbody>` as a body boundary, writes an explicit `<thead>`/`<tbody>`/`<tfoot>`
  grouping as row counts instead of reporting it unspellable, keeps the table's
  ID and classes, and still reports dropped table attributes when every row is
  blank (markup-carve/carve-rs#2273, markup-carve/carve-rs#2274,
  markup-carve/carve-rs#2275, markup-carve/carve-rs#2278,
  markup-carve/carve-rs#2279).
- A named colon fence with invalid opener metadata stays a container with its
  children parsed, and `lintCarve` reports `fence-title-syntax`, where the
  fence used to reach the page as text (markup-carve/carve-rs#2260).
- References resolve in authored AST children such as ruby content, short
  captions and figure images, literal definitions inside verse are kept, and an
  indented comment closer ends the comment inside a colon fence
  (markup-carve/carve-rs#2253, markup-carve/carve-rs#2256).
- Citations skip an invalid `@` in prefix text, honor backslash escapes before
  `@` and the suppress-author `-`, and keep `[@a , p. 4]` literal as the other
  engines do (markup-carve/carve-rs#2267).

### Improvements

- The npm tarball includes Node, browser `/web` and rendering-only `/render`
  entries, plus `/offsets` conversion helpers and a module worker example.
  The full entries share one WASM payload.
- `ParserSession` retains parsing state, reports parsed bytes and reuses
  unchanged plain paragraphs. Invalid edits leave its document unchanged.
- Release checks install the tarball, compile a TypeScript consumer, test Node
  ESM and CommonJS, and exercise Vite bundles and workers in Chromium, Firefox
  and WebKit. Benchmarks measure startup, rendering, edits, size and memory.

- The embedded engine uses a pinned Git revision based on `carve-lang` 0.1.8,
  up from 0.1.7, and renders every
  one of the 2215 corpus documents the pinned spec declares byte-identically
  (markup-carve/carve-rs#2252, markup-carve/carve-rs#2295).
- Parsing and HTML rendering allocate and rescan far less: nested colon
  bodies, quoted fences, list markers, citations, reference definitions,
  heading IDs and nested quotes each stop re-reading what an enclosing level
  already scanned, and plain Unicode paragraphs take the fast source-to-HTML
  path (markup-carve/carve-rs#2240, markup-carve/carve-rs#2241,
  markup-carve/carve-rs#2257, markup-carve/carve-rs#2263,
  markup-carve/carve-rs#2266, markup-carve/carve-rs#2268,
  markup-carve/carve-rs#2272, markup-carve/carve-rs#2277,
  markup-carve/carve-rs#2280, markup-carve/carve-rs#2281,
  markup-carve/carve-rs#2282, markup-carve/carve-rs#2283,
  markup-carve/carve-rs#2285, markup-carve/carve-rs#2286,
  markup-carve/carve-rs#2291).

## [0.1.5] - 2026-09-29

### Changed

- **Breaking for a `parseJson` consumer:** a footnote reference node spells its
  target as `label`, where it spelled it `id`. PART 12 section 25 settles that
  name on the definition, and every node also carries `attrs.id` for an authored
  `{#x}`, so the old name stood for two unrelated values on one object.
  `attrs.id` is untouched (markup-carve/carve-rs#1853).
- **Breaking for a `parseJson` consumer:** a code block's `content` is the
  literal payload text, so `a`, `a\n` and `a\n\n` stay distinct on the wire
  where they collapsed to one shape, and an empty fence holds no newline.
  Rendered HTML is unchanged (markup-carve/carve-rs#2191,
  markup-carve/carve-rs#2195).
- **Breaking for a `toMarkdown` consumer:** the Markdown target follows PART 11
  section 11 for a GFM reader. A heading takes no `{#id}` suffix, and a resolved
  cross-reference is written with the heading's GFM slug, so
  `lowercaseHeadingIds` no longer changes that anchor - the slug is the one a GFM
  reader computes for itself. `toHtml` still answers to the option
  (markup-carve/carve-rs#2014).

### Fixes

- `htmlToCarve` and `htmlToAst` no longer hand back a denied-scheme destination
  from `safe` mode. The published 0.1.4 imports
  `<a href="javascript:alert(1)">x</a>` as a live link and reports no
  diagnostic; the destination is now dropped, the content kept, and one
  `attribute-dropped` row names what went. `carve-lang` 0.1.7 carries the rule,
  so the stand-in copy this wrapper held goes with the bump (#145, #148).
- A render error encodes the report's fields directly instead of building an
  engine result for the encoder to read four fields off. A field added upstream
  no longer stops this wrapper compiling at the pin bump rather than at the
  change that caused it (#147).

### Improvements

- The embedded engine is `carve-lang` 0.1.7, up from 0.1.6, and renders every one
  of the 2134 corpus documents the pinned spec declares byte-identically. A
  non-breaking space reaches the ProseMirror bridge as its own
  `degraded:non_breaking_space` row rather than folded into the smart-punctuation
  one (#149).
- The embedded engine is the published `carve-lang` crate at an exact version
  rather than the same commit fetched from git, so a build resolves through
  crates.io and the corpus gate resolves the spec through the matching carve-rs
  release tag, which it now refuses unless that tag's `Cargo.toml` declares the
  pinned version (#136, #143).

## [0.1.4] - 2026-09-19

### Added

- `astJsonToMarkdown`, `astJsonToPlainText` and `astJsonToAnsi` render an
  AST-JSON document to those targets directly. Each takes the same options
  object the source-taking entry points read, and runs the profile filter and
  the `before_render` hooks. Definition ordering follows the tree, so a tree
  without spans prints them in label order. Behind `ast-json` and
  `other-renderers`, the pair `astJsonToCarve` already sits behind.
  markup-carve/carve-wasm#130

- Version 2 importer-fidelity reports for HTML and Markdown, plus additive
  `migrateDjot` and `migrateBbcode` entry points that retain the legacy string
  converter APIs.
- `mentionUrl` and `tagUrl` on the render options object, the engine's URL
  templates for `@mention` and `#tag`. Without them both render as inert spans,
  where carve-js links them (#72, #97).
- `applyProfile`, which runs a profile over an AST-JSON document and returns the
  filtered tree alongside what the filter degraded or stripped. The HTML path
  discards that (#100).
- The ProseMirror bridge, `toProseMirror` and `fromProseMirror`, reporting per
  node type what the editor model could not hold (#101).
- `toHtmlWithRenderers`, taking `renderers.math` for `mode: 'static'` and
  returning `{ html, rendererErrors }`. Static output carries no client scripts,
  so a formula has to be typeset while the HTML is written (#106).
- `toMarkdownWithOptions`, `toPlainTextWithOptions`, `toAnsiWithOptions`,
  `toCarveWithOptions` and `parseJsonWithOptions`. The options object reached
  HTML only, so a host had no way to hold an untrusted document to a `profile`
  on any other target (#108).
- `lintCarveWithOptions`, `lintAccessibility`, `stampCarve`, `sanitizeSvg`,
  `parseLocator`, `parseSourceLayoutJson` and `markdownToAstJson` - engine
  capability that had no binding (#109).
- `renderers.diagrams` on that same call, keyed by the fence's css class, each
  value the `(source) => html` callback `renderers.math` takes. A failure joins
  `rendererErrors` with `renderer` naming the class. Reading the key threw
  before, while its shape was undecided (#105, #120).
- `expandIncludes`, resolving `{{ path }}` directives through a host resolver
  and handing back the expanded tree as AST JSON. The resolver is SYNCHRONOUS,
  so a browser resolving over the network cannot use it, and an async one lands
  in `resolverErrors` rather than being swallowed. `maxBytes`, `maxDepth`,
  `maxResolverCalls` and `maxWarnings` are exposed with their defaults
  (#115, #121).
- `parseSnapshot` and `reparse`, each returning `{ source, document,
  sourceLayout, changedSource, reusedPreviousTree }` as one JSON string. The
  change offsets are UTF-8 BYTES, which is what `parseJson` positions and
  `createSourcePatch` ranges already mean, so a host counting UTF-16 code units
  converts first and a range splitting a character is refused (#111, #122).
- `createAstPatch`, `applyAstPatch`, `createReversibleAstPatch` and
  `applyReversibleAstPatch`, with trees and patches both crossing as JSON
  strings. Replaying a reversible patch onto a tree whose fingerprint is not the
  one it was made against throws, so an undo cannot land on a document that has
  moved on (#112, #125).
- `mergeAst`, taking an optional synchronous `options.resolve`. A conflict is a
  value, `{ ok: false, ast: null, conflicts }`, rather than an exception, under
  the three reason names carve-js uses. An async resolver is reported in
  `resolverErrors` and its conflict left unresolved (#113, #126).
- `htmlToAst`, returning the tree in the `{ value, report }` shape `htmlToCarve`
  already hands back. The two reports differ: a loss only the writer takes is
  absent here, so a `<figure>` wrapping a table reports `structure-unspellable`
  from `htmlToCarve` and not from this (#114, #127).

### Changed

- Pinned the Rust engine to its importer-fidelity v2 implementation. Markdown
  and the new report-returning Djot/BBCode APIs fail closed when fidelity is
  not yet assessed at construct level.
- A substitution node carries `old` and `new`, each an array of inline nodes,
  where it carried the strings `oldText` and `newText`. A host reading an
  AST-JSON tree for substitutions walks the halves instead of reading them
  (markup-carve/carve-rs#1756). Breaking for that host; nothing else in the tree moved.
- Advances the embedded carve-rs revision to released 0.1.6 (`d7837249`, from
  `9a0c421d`). The Markdown and Carve writers escape more of what would reopen a
  construct on the way back in: a literal tilde, an underscore pair the line
  would pair, a leading hash, a heading's trailing hash run, a caret before a
  bracket node, and the colon of a trailing `:name` (markup-carve/carve-rs#1641,
  markup-carve/carve-rs#1653, markup-carve/carve-rs#1681, markup-carve/carve-rs#1687, markup-carve/carve-rs#1713, markup-carve/carve-rs#1728).
  Parsing tightens around braced inlines, forced closers, escaped markers,
  adjacent links, blank table rows and a code span's closer (markup-carve/carve-rs#1638,
  markup-carve/carve-rs#1673, markup-carve/carve-rs#1675, markup-carve/carve-rs#1677, markup-carve/carve-rs#1684, markup-carve/carve-rs#1727,
  markup-carve/carve-rs#1745, markup-carve/carve-rs#1749, markup-carve/carve-rs#1753). A frontmatter block now survives
  the ProseMirror bridge as written (markup-carve/carve-rs#1695), and the bridge reports what
  a mention loses rather than dropping it silently (markup-carve/carve-rs#1760,
  markup-carve/carve-rs#1764, markup-carve/carve-rs#1766, markup-carve/carve-rs#1770, markup-carve/carve-rs#1773). HTML output over
  the spec corpus is unchanged: 1740/1740 documents byte-identical.

## [0.1.3] - 2026-09-08

### Added

- Source-preserving patch creation, canonical-format preview, and safe
  application through `createSourcePatch`, `toCarvePatch`, and
  `applySourcePatch`, over the shared Rust engine wire contract.

### Fixed

- Advances the embedded carve-rs revision to released 0.1.5 (`56cb3536`, from
  `da45f9d2`). A lone `|` line no longer panics, closing an unauthenticated
  one-byte denial of service for an embedder rendering untrusted Carve
  (markup-carve/carve#1554). The same range also brings parse-parity fixes it carries:
  comment leaf spans, emptied definition descriptions, footnote nesting, a
  nested item's leading fence, and all-blank and lone-pipe table rows. Measured
  through the built artifact: 1685/1685 corpus documents byte-identical at the
  spec commit this engine pins.


## [0.1.2] - 2026-09-04

### Added

- `rawHtml` on the `toHtmlWithOptions` object: `false` renders an explicit
  passthrough - the `=html` block and the `` `…`{=html} `` span - as escaped
  text. The switch carve-js spells `allowRawHtml`, and the one a host needs to
  render a document it did not author (#70, #71).
- `profile` (`full` / `article` / `comment` / `minimal`) and `profileBaseHost`,
  for the rest of that story: input length, denied constructs, link policy. A
  rejected document THROWS an `Error` named `ProfileViolationError` carrying
  `violations`, where the engine's infallible entry point would have returned an
  empty string (#73, #77).
- `mode`, `sourceLine`, `positions`, `labels`, `smartTypography`,
  `lowercaseHeadingIds` and `asciiHeadingIds` on the same object, closing the
  gap to the options carve-rb already passed (#74, #75, #77).
- `astJsonToHtml` and `astJsonToCarve`, which take an AST-JSON document back in.
  `parseJson` could write a tree out and nothing could render or save an edited
  one (#76, #78).
- `lintCarve`, `readStamp`, `needsReview`, `fromDjot` and `fromBbcode` - engine
  capabilities that had no binding at all (#76, #78).
- Optional capability features and a rendering-only web build profile. The
  full API remains enabled by default; `--no-default-features` keeps the HTML
  renderers used by the Playground while cutting the measured gzip payload
  roughly in half (#67).

### Changed

- Advances the embedded carve-rs revision to `da45f9d2`, matching the current
  carve-rb floor. Djot migration now preserves table continuations
  (markup-carve/carve-rs#1478), empty external-link targets are omitted (markup-carve/carve-rs#1479),
  titled media emits one title attribute (markup-carve/carve-rs#1481), authored task states
  survive format cycles and extended states name themselves in HTML
  (markup-carve/carve-rs#1485, markup-carve/carve-rs#1486), and a colon followed by a space and a tab no
  longer opens a description (markup-carve/carve-rs#1488).

## [0.1.1] - 2026-08-27

### Added

- Checked `to*WithReport` exports for every render target (markup-carve/carve#1728), with bounded positioned losses and strict refusal.
- Shared `fromHtml` and `fromMarkdown` migration entry points with reports.
- `toMarkdown`, `toPlainText`, `toAnsi`, and `toCarve`, completing the core
  carve-rs render-target surface exposed by the WASM package.

### Changed

- Advances the embedded carve-rs revision from `9cf16d05` to released 0.1.4
  (`2e9c43f2`), matching the current sibling binding floor. Documents that used to come
  back rendering differently through the HTML importer now survive it: a task
  item comes back a task item rather than as the checkbox HTML that rendered it
  (markup-carve/carve-rs#1366, markup-carve/carve-rs#1364, markup-carve/carve-rs#1374), and a grouping label keeps its
  div fence (markup-carve/carve-rs#1322). Attached list-marker attributes are now
  layout-transparent, so item bodies use the bare marker's content column
  (markup-carve/carve#1701). Measured through the built artifact: 1394/1394
  corpus documents byte-identical at the spec commit this engine pins.
- Embeds carve-rs at `9cf16d05` instead of `9705274c`, 105 commits later and 32 past the revision carve-rb embeds, so the two bindings of this engine no longer render the same document differently (#51, #56). On top of what `85514c6b` already carried - rendered elements saying what they are called (PART 9 §16a), `<thead>` and `<tfoot>` writing one row per line, a table cell's marker run ending at a space - this run is mostly the HTML importer: a deletion, a math span and a div's content survive an import, an import rebuilds the container the renderer wrote and takes the labels map the HTML was rendered with, and a container's span ends where its markup does. Measured through the built artifact: 1371/1371 mandatory corpus documents byte-identical at the spec commit this engine pins.

### Added

- `toMarkdown`, `toPlainText`, `toAnsi` and `toCarve`, so every core render
  target the embedded engine already understood is reachable from JavaScript
  (#54). `toHtml` and the AST entry points are unchanged, and no rebuild and no
  engine bump is involved: the flags were inside the shipped artifact and had no
  binding.
- `scripts/check-engine-floor.py`, run by CI, fails when the engine pin falls behind the revision carve-rb embeds. The age check passes on a pin bumped inside its window however far behind a sibling it is, and the corpus gate is aimed at the spec commit the pinned engine pins, so an old pin and an old spec stay green together - which is how this pin got 28 commits behind (#51).

## [0.1.0] - 2026-08-18

First release. Nothing has been published to npm before this, so there is no
version a reader can be upgrading from.

- WebAssembly bindings for the Carve parser and HTML renderer: `toHtml`, the
  extension surface, the `sections` option, the static render mode, and the
  parsed AST.
- Embeds carve-rs at `9705274c` (crate version `0.1.3`, 33 commits past the
  `0.1.3` tag), which includes the PART 9 §25 fix where a list-valued URL
  attribute is probed at every candidate rather than at its head -
  `srcset="safe.png 1x, javascript:alert(1) 2x"` used to pass on its second
  entry.
- The publish is gated: `.github/workflows/release.yml` runs
  `scripts/verify-release-artifact.mjs` against the packed npm tarball and the
  publish job declares that gate in `needs:`, so a tarball that renders the spec
  corpus differently cannot reach the registry.

[Unreleased]: https://github.com/markup-carve/carve-wasm/compare/v0.1.5...HEAD
[0.1.5]: https://github.com/markup-carve/carve-wasm/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/markup-carve/carve-wasm/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/markup-carve/carve-wasm/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/markup-carve/carve-wasm/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/markup-carve/carve-wasm/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/markup-carve/carve-wasm/releases/tag/v0.1.0
