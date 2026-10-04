// A destination the sink denylist blanks owes one `destination-denied` row on
// the render-loss report, and the HTML keeps the emptied attribute.
import assert from 'node:assert/strict'
import { toHtmlWithOptions, toHtmlWithReport } from './engine.mjs'

const rows = (result) => result.losses.map(({ code, target, nodeType, message }) => ({ code, target, nodeType, message }))
const link = { code: 'destination-denied', target: 'html', nodeType: 'inline', message: 'Blanked a denied destination scheme' }
const image = { code: 'destination-denied', target: 'html', nodeType: 'inline', message: 'Blanked a denied image source' }

// `toHtmlWithReport` renders with raw HTML allowed, which is the `safe=false`
// path: the flag does not gate the blanking, so it does not gate the row.
for (const [name, source, html, expected] of [
  ['denied link', '[x](javascript:alert(1))', '<p><a href="">x</a></p>', [link]],
  ['denied autolink', '<javascript:alert(1)>', '<p><a href="">javascript:alert(1)</a></p>', [link]],
  ['denied image', '![a](javascript:alert(1))', '<img src="" alt="a">', [image]],
  ['allowed URL', '[x](https://ok.example)', '<p><a href="https://ok.example">x</a></p>', []],
]) {
  const result = toHtmlWithReport(source)
  assert.equal(result.value, html, `${name}: the HTML is unchanged`)
  assert.deepEqual(rows(result), expected, `${name}: one row per blanked destination`)
  assert.equal(result.totalLosses, expected.length, `${name}: totalLosses`)
  assert.equal(result.truncated, false)
  if (expected.length) assert.equal(result.losses[0].pos.startLine, 1, `${name}: the row carries a position`)
  // The safe render blanks the same destination byte for byte.
  assert.equal(toHtmlWithOptions(source, { rawHtml: false }), html, `${name}: safe mode emits the same HTML`)
}

assert.throws(
  () => toHtmlWithReport('[x](javascript:alert(1))', true),
  (error) => error.name === 'RenderLossError'
    && error.message === 'render would lose 1 node'
    && error.totalLosses === 1
    && error.losses.length === 1
    && error.losses[0].code === 'destination-denied',
)
assert.throws(
  () => toHtmlWithReport('[x](javascript:one) ![a](vbscript:two)', true),
  (error) => error.name === 'RenderLossError' && error.message === 'render would lose 2 nodes',
)
assert.deepEqual(toHtmlWithReport('[x](https://ok.example)', true).losses, [])

console.log('wasm artifact: destination-denied render losses pass')
