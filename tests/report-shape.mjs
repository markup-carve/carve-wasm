// The render-loss report has to carry the engine's own per-code totals.
//
// `losses` is bounded by `maximum`, so counting codes off that array is wrong
// the moment the report truncates. `totalsByCode` is the engine's answer and it
// is the only field that survives truncation intact.
import assert from 'node:assert/strict'
import { toAnsiWithReport, toCarveWithReport, toHtmlWithReport, toMarkdownWithReport, toPlainTextWithReport } from './engine.mjs'

const denied = (n) => `[x](javascript:a${n})\n\n`
const raw = (n) => '```=weird\nraw' + n + '\n```\n\n'

// Under the cap: the totals agree with what a caller could have counted.
const small = toHtmlWithReport(denied(1) + raw(1), false, null)
assert.deepEqual(small.totalsByCode, { 'destination-denied': 1, 'raw-format-dropped': 1 })
assert.equal(small.totalLosses, 2)
assert.equal(small.truncated, false)

// Over the cap, where counting the array is wrong. 80 denied destinations and
// 40 raw blocks, reported 100: the visible split is 80/20 and the real one is
// 80/40.
let source = ''
for (let i = 0; i < 80; i++) source += denied(i)
for (let i = 0; i < 40; i++) source += raw(i)
const big = toHtmlWithReport(source, false, null)
assert.equal(big.truncated, true)
assert.equal(big.totalLosses, 120)
assert.equal(big.losses.length, 100)
const visible = {}
for (const { code } of big.losses) visible[code] = (visible[code] ?? 0) + 1
assert.deepEqual(visible, { 'destination-denied': 80, 'raw-format-dropped': 20 }, 'the premise: the array under-reports')
assert.deepEqual(big.totalsByCode, { 'destination-denied': 80, 'raw-format-dropped': 40 }, 'the totals do not')

// A refused strict render carries them too. That is the path where a caller has
// no `losses` to count in the first place.
assert.throws(
  () => toHtmlWithReport(denied(1) + raw(1), true),
  (error) => {
    assert.equal(error.name, 'RenderLossError')
    assert.deepEqual(error.totalsByCode, { 'destination-denied': 1, 'raw-format-dropped': 1 })
    return true
  },
)

// Every target that reports, not only HTML: one encoder serves all five, and a
// field added to one of them by hand is how this one went missing.
for (const [name, render] of [
  ['toMarkdownWithReport', toMarkdownWithReport],
  ['toPlainTextWithReport', toPlainTextWithReport],
  ['toAnsiWithReport', toAnsiWithReport],
  ['toCarveWithReport', toCarveWithReport],
]) {
  const result = render(raw(1), false, null)
  assert.equal(typeof result.totalsByCode, 'object', `${name} carries totalsByCode`)
  assert.notEqual(result.totalsByCode, null, `${name} carries totalsByCode`)
  const counted = Object.values(result.totalsByCode).reduce((sum, n) => sum + n, 0)
  assert.equal(counted, result.totalLosses, `${name}: the totals sum to totalLosses`)
}

// A clean render reports an empty map rather than leaving the field out, so a
// consumer can read it without a guard.
assert.deepEqual(toHtmlWithReport('plain\n', false, null).totalsByCode, {})

console.log('wasm artifact: render-loss report shape passes')
