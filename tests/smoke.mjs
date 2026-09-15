// Exercise the built WASM ARTIFACT through the JS API users actually load.
// `cargo test` only tests the native build of the wrapper; it never proves the
// wasm package works -- which is how a stale engine can pass CI (the embedded
// engine in carve-go did exactly that for months).
import assert from 'node:assert/strict'
import {
  astJsonToCarve,
  astJsonToHtml,
  applyProfile,
  applySourcePatch,
  createSourcePatch,
  htmlToCarve,
  fromBbcode,
  fromDjot,
  fromProseMirror,
  fromMarkdown,
  migrateBbcode,
  migrateDjot,
  lintCarve,
  needsReview,
  parseJson,
  readStamp,
  toHtml,
  toHtmlWithOptions,
  toHtmlWithReport,
  toProseMirror,
  toCarvePatch,
} from './engine.mjs'

const cases = [
  // Superscript/subscript are braced-only: a bare `^` / `,` is literal text.
  ['a ^2^ b', '<p>a ^2^ b</p>'],
  ['x{^2^}', '<p>x<sup>2</sup></p>'],
  ['H,2,O', '<p>H,2,O</p>'],
  ['H{,2,}O', '<p>H<sub>2</sub>O</p>'],
  // A symbol needs a leading word boundary, so these stay literal.
  ['a:b:c and 10:30: here', '<p>a:b:c and 10:30: here</p>'],
  ['mail me@example.com now', '<p>mail me@example.com now</p>'],
  // Core sanity.
  ['# Title', '<section id="Title">\n  <h1>Title</h1>\n</section>'],
  ['a *bold* b', '<p>a <strong>bold</strong> b</p>'],
]

let failed = 0
for (const [src, want] of cases) {
  const got = toHtml(src).trim()
  if (got !== want) {
    console.error(`FAIL ${JSON.stringify(src)}\n  got:  ${got}\n  want: ${want}`)
    failed++
  }
}
assert.equal(failed, 0, `${failed} wasm artifact case(s) failed`)
console.log(`wasm artifact: ${cases.length}/${cases.length} cases pass`)

const source = 'see → here'
const sourcePatch = createSourcePatch(source, 'see ⇒ here', 'quick-fix', 'arrow-style')
assert.equal(sourcePatch.sourceBytes, Buffer.byteLength(source))
assert.equal(sourcePatch.edits[0].kind, 'quick-fix')
assert.equal(applySourcePatch(source, sourcePatch), 'see ⇒ here')
assert.throws(() => applySourcePatch('stale', sourcePatch))
assert.equal(applySourcePatch('# Title   ', toCarvePatch('# Title   ')), '# Title\n')
for (const kind of ['formatting', 'syntax-migration', 'quick-fix', 'refactor']) {
  const wire = JSON.parse(JSON.stringify(createSourcePatch('a', 'b', kind, 'smoke-test')))
  assert.equal(wire.edits[0].kind, kind)
  assert.equal(applySourcePatch('a', wire), 'b')
}
assert.deepEqual(createSourcePatch('same', 'same', 'refactor', 'no-op').edits, [])
assert.throws(() => createSourcePatch('a', 'b', 'unknown', 'bad-kind'), TypeError)
assert.throws(() => createSourcePatch('a', 'b', 'refactor', ''), TypeError)
console.log('wasm artifact: source patches pass')

const checked = toHtmlWithReport('`x`{=latex}', false, 1)
assert.equal(checked.value, '<p></p>')
assert.deepEqual(checked.losses.map(({ code, format, target, nodeType }) => ({ code, format, target, nodeType })), [
  { code: 'raw-format-dropped', format: 'latex', target: 'html', nodeType: 'inline' },
])
assert.equal(checked.losses[0].pos.startLine, 1)
assert.throws(
  () => toHtmlWithReport('`x`{=latex}', true, 1),
  (error) => error.name === 'RenderLossError' && error.totalLosses === 1 && error.losses.length === 1,
)
console.log('wasm artifact: checked render losses pass')

// The `sections` option, through the artifact. `cargo test` covers the Rust
// renderers directly; only this proves the option survives the wasm-bindgen
// boundary, where a boolean has to cross as a JS value.
assert.equal(
  toHtmlWithOptions('# A\n\np\n', { sections: false }).trim(),
  '<h1 id="A">A</h1>\n<p>p</p>',
)
// Omitted, null and an empty object all mean "defaults", so a caller may pass a
// partially-filled object.
for (const opts of [undefined, null, {}, { sections: true }]) {
  assert.equal(
    toHtmlWithOptions('# A\n', opts).trim(),
    '<section id="A">\n  <h1>A</h1>\n</section>',
    `defaults expected for ${JSON.stringify(opts) ?? 'undefined'}`,
  )
}
// Composes with the other two fields rather than being exclusive with them.
const composed = toHtmlWithOptions('# A\n\n:rocket:\n', {
  sections: false,
  symbols: { rocket: '🚀' },
  full: true,
}).trim()
// `full: true` enables heading permalinks, so the <h1> carries an anchor after
// its text - the id must still be on the <h1> itself, which is where that
// anchor's own href points (markup-carve/carve-rs#379).
assert.ok(composed.startsWith('<h1 id="A">A '), composed)
assert.ok(composed.includes('href="#A"'), composed)
assert.ok(composed.includes('🚀'), composed)
assert.ok(!composed.includes('<section'), composed)
// A recognized key with the wrong TYPE throws rather than being coerced: JS
// truthiness would read the string "false" as true, the opposite of what was
// written. An UNrecognized key is ignored - config typos should not break a
// render.
assert.throws(() => toHtmlWithOptions('# A\n', { sections: 'false' }), TypeError)
// Same contract for `symbols`: a wrong type throws instead of quietly rendering
// without the map, which would lose the caller's configuration silently.
assert.throws(() => toHtmlWithOptions('# A\n', { symbols: 'rocket' }), TypeError)
assert.throws(() => toHtmlWithOptions('# A\n', { symbols: 1 }), TypeError)
assert.equal(
  toHtmlWithOptions('# A\n', { sctions: false }).trim(),
  '<section id="A">\n  <h1>A</h1>\n</section>',
)
console.log('wasm artifact: sections option passes')

// The `rawHtml` option, through the artifact. This is the switch that lets a
// host render a document it did not author: a passthrough is the one construct
// that can put author-controlled markup on the host's origin.
const RAW_SOURCE = '```=html\n<img src=x onerror=alert(1)>\n```\n\nan `<b>x</b>`{=html} span\n'
const rawOn = toHtmlWithOptions(RAW_SOURCE, {}).trim()
assert.ok(rawOn.includes('<img src=x onerror=alert(1)>'), rawOn)
assert.ok(rawOn.includes('<b>x</b>'), rawOn)
const rawOff = toHtmlWithOptions(RAW_SOURCE, { rawHtml: false }).trim()
assert.ok(rawOff.includes('&lt;img src=x onerror=alert(1)&gt;'), rawOff)
assert.ok(!rawOff.includes('<img src=x'), rawOff)
assert.ok(rawOff.includes('&lt;b&gt;x&lt;/b&gt;'), rawOff)
// Composes with the extension path, which builds its options separately, and
// leaves the symbol map's TRUSTED-RAW contract alone: `rawHtml` is about the
// document's passthrough, not about what the host configured.
const rawFull = toHtmlWithOptions('```=html\n<b>raw</b>\n```\n\n:bold:\n', {
  rawHtml: false,
  full: true,
  symbols: { bold: '<b>x</b>' },
}).trim()
assert.ok(rawFull.includes('&lt;b&gt;raw&lt;/b&gt;'), rawFull)
assert.ok(rawFull.includes('<b>x</b>'), rawFull)
// Same wrong-type contract as the other booleans.
assert.throws(() => toHtmlWithOptions('# A\n', { rawHtml: 'false' }), TypeError)
console.log('wasm artifact: rawHtml option passes')

// The profile, through the artifact. A rejection has to ARRIVE as an error: the
// engine's infallible entry point turns one into an empty string, which a
// caller cannot tell from a document that rendered to nothing.
const rejected = (() => {
  try {
    return { html: toHtmlWithOptions('| a | b |\n|---|---|\n| 1 | 2 |\n', { profile: 'minimal' }) }
  } catch (error) {
    return { error }
  }
})()
if (rejected.error) {
  assert.equal(rejected.error.name, 'ProfileViolationError')
  assert.ok(Array.isArray(rejected.error.violations), 'violations should be an array')
  assert.ok(rejected.error.violations.length > 0, 'violations should not be empty')
} else {
  assert.notEqual(rejected.html, '', 'a rejection must not arrive as an empty string')
}
assert.ok(toHtmlWithOptions('# A\n', { profile: 'full' }).includes('<h1'))
assert.throws(() => toHtmlWithOptions('# A\n', { profile: 'nope' }), TypeError)

// Editor-preview switches.
assert.ok(!toHtmlWithOptions('# A\n\np\n', {}).includes('data-source-line'))
assert.ok(toHtmlWithOptions('# A\n\np\n', { sourceLine: true }).includes('data-source-line'))

// Output-shaping switches: the i18n seam, typography, and the slug policy.
assert.ok(toHtmlWithOptions('::: note\nbody\n:::\n', {}).includes('Note'))
assert.ok(
  toHtmlWithOptions('::: note\nbody\n:::\n', {
    labels: { admonitionNote: 'Hinweis' },
  }).includes('Hinweis'),
)
assert.ok(toHtmlWithOptions('a...b\n', { smartTypography: 'source' }).includes('a...b'))
assert.ok(
  toHtmlWithOptions('# Grüße Alle\n', {
    lowercaseHeadingIds: true,
    asciiHeadingIds: 'strict',
  }).includes('id="grusse-alle"'),
)
// Social-token URL templates. Without them a mention and a tag are inert spans,
// which is the whole reason a host has to be able to say where they point.
{
  const bare = toHtmlWithOptions('Hi @ada and #rust\n', {})
  assert.ok(!bare.includes('<a'), 'mentions and tags are inert without a template')
  const linked = toHtmlWithOptions('Hi @ada and #rust\n', {
    mentionUrl: 'https://example.com/users/{name}',
    tagUrl: 'https://example.com/tags/{name}',
  })
  assert.ok(linked.includes('href="https://example.com/users/ada"'), linked)
  assert.ok(linked.includes('href="https://example.com/tags/rust"'), linked)
  // The name is the document's, not the host's: it is encoded into the
  // template rather than concatenated onto it.
  assert.ok(
    toHtmlWithOptions('@a b\n', { mentionUrl: 'https://example.com/{user}' }).includes(
      'href="https://example.com/a"',
    ),
  )
}
assert.throws(() => toHtmlWithOptions('# A\n', { mentionUrl: 7 }), TypeError)
assert.throws(() => toHtmlWithOptions('# A\n', { tagUrl: 7 }), TypeError)

// `mode: 'static'` is accepted and renders; what it flattens is the engine's
// business, covered there.
assert.ok(toHtmlWithOptions('# A\n', { mode: 'static' }).includes('<h1'))
assert.throws(() => toHtmlWithOptions('# A\n', { mode: 'nope' }), TypeError)
assert.throws(() => toHtmlWithOptions('# A\n', { labels: 'Hinweis' }), TypeError)
assert.throws(() => toHtmlWithOptions('# A\n', { sourceLine: 'yes' }), TypeError)
console.log('wasm artifact: profile, editor and output-shaping options pass')

// The round trip. `parseJson` writes a tree out; until `astJsonToHtml` there was
// no way to render an edited one back, which is the reason to read a tree in a
// browser at all.
const treeSource = '# Title\n\nBody with _emphasis_.\n'
const tree = parseJson(treeSource)
assert.ok(astJsonToHtml(tree).includes('<h1'), 'a tree should render')
assert.equal(astJsonToCarve(tree).trim(), treeSource.trim())
// The options object reaches the tree path too, so an edited tree renders under
// the switches the host already configured.
assert.ok(!astJsonToHtml(tree, { sections: false }).includes('<section'))
assert.throws(() => astJsonToHtml('{"type":"nope"}'), Error)

// One options object has to mean the same thing on both paths. The tree
// renderer applies neither the profile filter nor the before-render hooks by
// itself, so rendering a tree straight through it made `profile` silently
// inert and `full` silently do nothing - the same call, two safety postures.
const parityDoc = '# H\n\ntext\n'
assert.equal(
  astJsonToHtml(parseJson(parityDoc), { full: true }),
  toHtmlWithOptions(parityDoc, { full: true }),
)
const parityTable = '| a | b |\n|---|---|\n| 1 | 2 |\n'
const treeFiltered = astJsonToHtml(parseJson(parityTable), { profile: 'minimal' })
assert.equal(treeFiltered, toHtmlWithOptions(parityTable, { profile: 'minimal' }))
assert.ok(!treeFiltered.includes('<table'), 'the profile has to filter on the tree path too')

// The filter on its own, with the tree kept. Until now a host could only get a
// profile applied on the way to HTML, and the HTML path discards what the filter
// did.
const denied = parseJson('a [home](https://example.com/x) b\n')
const stripped = applyProfile(denied, 'minimal')
assert.deepEqual(
  stripped.violations.map(({ nodeType, reason }) => ({ nodeType, reason })),
  [{ nodeType: 'link', reason: 'element_not_allowed' }],
)
assert.equal(
  stripped.violations[0].reasonDescription,
  'Links are disabled in this minimal context.',
)
assert.equal(
  stripped.violations[0].message,
  "'link' is not allowed: element_not_allowed (Links are disabled in this minimal context.)",
)
// A tree, not prose: the result feeds straight back into the entry points that
// take one.
assert.ok(!astJsonToHtml(stripped.json).includes('<a '), 'the link should be gone')
assert.equal(astJsonToCarve(stripped.json).trim(), 'a home b')

// A profile that denies nothing in this document reports nothing and changes
// nothing.
assert.deepEqual(applyProfile(denied, 'full').violations, [])
assert.equal(astJsonToHtml(applyProfile(denied, 'full').json), astJsonToHtml(denied))

// max_nesting is the filter's own bound, and it reports under a node type the
// document never denied.
const deep = applyProfile(parseJson('- a\n  - b\n    - c\n      - d\n'), 'minimal')
assert.ok(
  deep.violations.some(({ reason }) => reason === 'max_nesting_exceeded'),
  'minimal caps nesting at 2',
)

// The typography switch reaches the degrade. `to_text` decides the spelling of
// the text it substitutes, so a caller asking for the author's run has to get
// the author's run out of the FILTER, not only out of the renderer.
const dashed = parseJson('[x -- y](https://example.com/x)\n')
assert.equal(astJsonToHtml(applyProfile(dashed, 'minimal').json), '<p>x – y</p>')
assert.equal(
  astJsonToHtml(applyProfile(dashed, 'minimal', { smartTypography: 'source' }).json),
  '<p>x -- y</p>',
)

// Wrong types throw rather than being coerced, the way the render options do.
assert.throws(() => applyProfile(denied, 'nope'), TypeError)
assert.throws(() => applyProfile(denied, 'minimal', { smartTypography: 'nope' }), TypeError)
assert.throws(() => applyProfile(denied, 'minimal', { profileBaseHost: 7 }), TypeError)
assert.throws(() => applyProfile('{', 'full'), Error)

// The linter, with the rule ids carve-js and carve-php share.
assert.deepEqual(lintCarve('# Fine\n\ntext\n'), [])
const warnings = lintCarve('# Heading\n\n{.orphan}\n')
assert.equal(warnings.length, 1, 'a block attribute reaching no block should warn')
assert.equal(warnings[0].rule, 'unattached-block-attribute')
for (const key of ['line', 'column', 'rule', 'message', 'start', 'end']) {
  assert.ok(key in warnings[0], `a warning should carry ${key}`)
}

// Provenance.
assert.equal(readStamp('# Plain\n'), null)
assert.equal(needsReview('# Plain\n', '1.0.0'), true)

// The importers that had no binding. Djot swaps the emphasis delimiters, which
// is exactly why pasting Djot in as Carve renders wrongly rather than failing.
for (const result of [migrateDjot('_em_ and *strong*\n'), migrateBbcode('[b]bold[/b]')]) {
  assert.equal(result.report.schemaVersion, 2)
  assert.deepEqual(
    result.report.diagnostics.map(({ code, fidelity, confidence }) => ({ code, fidelity, confidence })),
    [{ code: 'fidelity-unverified', fidelity: 'dropped', confidence: 'fallback' }],
  )
}
assert.ok(fromDjot('_em_ and *strong*\n').includes('/em/'))
assert.ok(fromBbcode('[b]bold[/b]').includes('*bold*'))
const htmlMigration = htmlToCarve('<p onclick="x()">safe</p>', 'safe')
assert.equal(htmlMigration.report.schemaVersion, 2)
assert.equal(htmlMigration.report.sourceFormat, 'html')
assert.equal(htmlMigration.report.mode, 'safe')
assert.equal(htmlMigration.report.adapter, 'generic')
assert.ok(htmlMigration.report.diagnostics.some(({ code, fidelity, confidence }) =>
  code === 'attribute-dropped' && fidelity === 'dropped' && confidence === 'exact'))
const markdownMigration = fromMarkdown('**bold**')
assert.deepEqual(
  markdownMigration.report.diagnostics.map(({ code, fidelity, confidence }) => ({ code, fidelity, confidence })),
  [{ code: 'fidelity-unverified', fidelity: 'dropped', confidence: 'fallback' }],
)
console.log('wasm artifact: tree, lint, stamp and importer entry points pass')

// The ProseMirror bridge. ProseMirror runs in a browser and nowhere else, so
// this binding is the whole audience for the engine's bridge.
const pm = toProseMirror('A *bold /and italic/* word.')
// A JSON STRING, the same choice parseJson makes - the host parses it natively.
assert.equal(typeof pm.json, 'string')
assert.deepEqual(JSON.parse(pm.json), {
  type: 'doc',
  content: [
    {
      type: 'paragraph',
      content: [
        { type: 'text', text: 'A ' },
        { type: 'text', text: 'bold ', marks: [{ type: 'bold' }] },
        { type: 'text', text: 'and italic', marks: [{ type: 'bold' }, { type: 'italic' }] },
        { type: 'text', text: ' word.' },
      ],
    },
  ],
})
assert.deepEqual(pm.dropped, {})
assert.deepEqual(pm.degraded, {})

// A code block keeps its language as an attribute rather than in the text.
const fenced = JSON.parse(toProseMirror('``` rust\nlet x = 1;\n```\n').json).content[0]
assert.equal(fenced.type, 'codeBlock')
assert.equal(fenced.attrs.language, 'rust')
assert.deepEqual(fenced.content, [{ type: 'text', text: 'let x = 1;' }])

// What the ProseMirror model cannot hold is REPORTED, not silently converted,
// and the two maps say different things. `dropped` is content that is gone;
// `degraded` is content that survives without its node type. A soft break
// becomes whitespace, smart typography resolves to the glyph, so the author's
// `...` does not survive the trip back - and an abbreviation definition has no
// editor node at all.
const softBreak = toProseMirror('left\nright')
assert.equal(softBreak.degraded.soft_break, 'a soft break is whitespace in the ProseMirror model')
assert.deepEqual(softBreak.dropped, {})
assert.equal(
  toProseMirror('a ... b').degraded.smart_punctuation,
  'smart-typography output is lossy on reparse, so it is not modeled',
)
const abbreviated = toProseMirror('*[HTML]: HyperText Markup Language\n\nHTML is fine.\n')
assert.deepEqual(abbreviated.dropped, {
  abbreviation_def: "abbreviation definitions ride on the doc node's attrs",
})
assert.deepEqual(abbreviated.degraded, {})
assert.equal(fromProseMirror(toProseMirror('a ... b').json), 'a … b\n')

// The editor loop closes: source in, source back.
const editable = 'A *bold /and italic/* word.\n\n- one\n- two\n\n> quoted\n'
assert.equal(fromProseMirror(toProseMirror(editable).json), editable)

// A payload the schema map does not describe is refused rather than written
// approximately.
assert.throws(() => fromProseMirror('{"type":"nope"}'), Error)
assert.throws(() => fromProseMirror('{'), Error)
console.log('wasm artifact: ProseMirror bridge passes')




// The PART 12 exchange shape, through the same artifact. A binding that can only
// render is unusable for an editor, a linter or a converter - they need the tree.
const ast = JSON.parse(parseJson('---\ntitle: T\n---\n\nBody[^a].\n\n[^a]: note\n'))

// The root carries exactly three fields (PART 12 §7): frontmatter and footnote
// definitions are block nodes in the tree, not root fields.
assert.deepEqual(Object.keys(ast).sort(), ['children', 'srcByteLength', 'type'])
assert.deepEqual(
  ast.children.map((n) => n.type),
  ['frontmatter', 'paragraph', 'footnote'],
)
// Raw, not a parsed mapping - a parsed map cannot be serialized back to the
// bytes the author wrote.
assert.equal(ast.children[0].content, 'title: T')

// Positions are CODEPOINTS (§4). Bytes and UTF-16 units agree with codepoints
// below U+10000, so the astral character is what makes a wrong unit visible: the
// emoji is 4 bytes, 2 UTF-16 units and 1 codepoint, so only a codepoint index
// puts the strong at column 3.
const astral = JSON.parse(parseJson('\u{1F600} *b*\n'))
const strong = astral.children[0].children.at(-1)
assert.equal(strong.type, 'strong')
assert.equal(strong.pos.startColumn, 3)
assert.equal(strong.pos.startOffset, 2)

console.log('wasm artifact: AST cases pass')
