// What unit each diagnostic family counts in, pinned against a document where
// the three candidate units disagree.
//
// A JavaScript string is indexed in UTF-16 code units, so neither of the units
// the engine answers in can slice one. The rocket below is 4 bytes, 1 codepoint
// and 2 UTF-16 units, which is the only shape that tells all three apart: an
// ASCII document makes every unit the same number, which is why this was
// silently right for most callers and silently wrong for the rest.
import assert from 'node:assert/strict'
import { lintAccessibility, lintCarve, lintCarveWithOptions, parseJson } from './engine.mjs'

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

// The lint family counts UTF-16 CODE UNITS, so `start` and `end` index the
// JavaScript string the caller already holds. On this document the byte pair
// would be 6 and 18 and the codepoint pair 3 and 15, so a wrong unit cannot
// pass here.
assert.equal(warning.start, 4)
assert.equal(warning.end, 16)
assert.equal(lintSource.slice(warning.start, warning.end), '[t][missing]')

// The additive pair #159 shipped is gone from this family: it said the same
// thing as `start` / `end` once those became UTF-16, and it never reached a
// release. `lintAccessibility` keeps its own pair, whose offsets still differ.
assert.equal(warning.startUtf16, undefined)
assert.equal(warning.endUtf16, undefined)

// The option-taking entry point answers in the same unit. It is a separate
// export reaching the same encoder, so a flip applied to one and not the other
// would leave the two disagreeing.
const [withOptions] = lintCarveWithOptions(lintSource, null)
assert.equal(withOptions.start, 4)
assert.equal(withOptions.end, 16)
assert.equal(lintSource.slice(withOptions.start, withOptions.end), '[t][missing]')

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
assert.equal(plain.start, 0)
assert.equal(plain.end, 12)
assert.equal(ascii.slice(plain.start, plain.end), '[t][missing]')

console.log('wasm artifact: diagnostic offset units pass')
