// Exercise the built WASM ARTIFACT through the JS API users actually load.
// `cargo test` only tests the native build of the wrapper; it never proves the
// wasm package works -- which is how a stale engine can pass CI (the embedded
// engine in carve-go did exactly that for months).
import assert from 'node:assert/strict'
import { parseJson, toHtml, toHtmlWithOptions } from './engine.mjs'

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
