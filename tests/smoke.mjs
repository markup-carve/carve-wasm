// Exercise the built WASM ARTIFACT through the JS API users actually load.
// `cargo test` only tests the native build of the wrapper; it never proves the
// wasm package works -- which is how a stale engine can pass CI (the embedded
// engine in carve-go did exactly that for months).
import assert from 'node:assert/strict'
import { parseJson, toHtml } from '../pkg/carve_wasm.js'

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
