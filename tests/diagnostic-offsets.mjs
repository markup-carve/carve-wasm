// What unit each diagnostic family counts in, pinned against a document where
// the three candidate units disagree.
//
// A JavaScript string is indexed in UTF-16 code units, so neither of the units
// the engine answers in can slice one. The rocket below is 4 bytes, 1 codepoint
// and 2 UTF-16 units, which is the only shape that tells all three apart: an
// ASCII document makes every unit the same number, which is why this was
// silently right for most callers and silently wrong for the rest.
import assert from 'node:assert/strict'
import { lintAccessibility, lintCarve, parseJson } from './engine.mjs'

const unitsDiffer = (source) => {
  const bytes = Buffer.byteLength(source, 'utf8')
  const codepoints = [...source].length
  assert.notEqual(bytes, codepoints)
  assert.notEqual(codepoints, source.length)
  return source
}

const lintSource = unitsDiffer('\u{1F680}\n\n[t][missing]\n')
const [warning] = lintCarve(lintSource)
assert.equal(warning.rule, 'unresolved-reference-link')

// The existing pair is BYTES, and stays bytes. Pinned so a change of unit is a
// test failure rather than a silent shift.
assert.equal(warning.start, 6)
assert.equal(warning.end, 18)
assert.equal(
  Buffer.from(lintSource, 'utf8').subarray(warning.start, warning.end).toString('utf8'),
  '[t][missing]',
)

// The new pair indexes the JavaScript string the caller holds.
assert.equal(warning.startUtf16, 4)
assert.equal(warning.endUtf16, 16)
assert.equal(lintSource.slice(warning.startUtf16, warning.endUtf16), '[t][missing]')

// The accessibility family counts in CODEPOINTS, not bytes. Two lint APIs in
// one package, two units: that difference is the trap, and it is pinned here so
// the documentation and the behavior cannot drift apart.
const a11ySource = unitsDiffer('\u{1F680}\n\n![](x.png)\n')
const [diagnostic] = lintAccessibility(a11ySource)
assert.equal(diagnostic.rule, 'a11y/image-alt')
assert.equal(diagnostic.startOffset, 3)
assert.equal(diagnostic.endOffset, 13)
assert.equal([...a11ySource].slice(diagnostic.startOffset, diagnostic.endOffset).join(''), '![](x.png)')
assert.equal(diagnostic.startUtf16, 4)
assert.equal(diagnostic.endUtf16, 14)
assert.equal(a11ySource.slice(diagnostic.startUtf16, diagnostic.endUtf16), '![](x.png)')

// The AST keeps codepoints, which PART 12 section 4 pins for the wire format.
// Named here so a future sweep over "offsets should be UTF-16" does not take
// this one with it.
const paragraph = JSON.parse(parseJson(lintSource)).children[1]
assert.equal(paragraph.pos.startOffset, 3)

// An absent position stays null rather than becoming 0, in both pairs.
for (const item of lintAccessibility('# a\n\n### c\n')) {
  if (item.startOffset === null) assert.equal(item.startUtf16, null)
}

// On an ASCII document every unit coincides. This is the case that hid the bug,
// so it is worth asserting rather than assuming.
const ascii = '[t][missing]\n'
const [plain] = lintCarve(ascii)
assert.equal(plain.start, plain.startUtf16)
assert.equal(plain.end, plain.endUtf16)

console.log('wasm artifact: diagnostic offset units pass')
