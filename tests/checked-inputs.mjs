import assert from 'node:assert/strict'
import { reparse, expandIncludes, toHtmlWithReport, toMarkdownWithReport, toPlainTextWithReport, toAnsiWithReport, toCarveWithReport } from './engine.mjs'

for (const range of [[2 ** 32, 2 ** 32 + 1], [0, 2 ** 32 + 1], [Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER], [-1, 1], [0.5, 1], [0, 4]]) {
  assert.throws(() => reparse('abc', JSON.stringify([{ range, replacement: 'X' }])), TypeError, `range ${range}`)
}
for (const range of [[0, 1], [1, 2], [0, 3]]) {
  assert.throws(() => reparse('😀é', JSON.stringify([{ range, replacement: 'X' }])), TypeError)
}
assert.equal(JSON.parse(reparse('😀é', JSON.stringify([{ range: [4, 6], replacement: 'X' }]))).source, '😀X')
assert.equal(JSON.parse(reparse('abc', JSON.stringify([{ range: [0, 1], replacement: 'X' }]))).source, 'Xbc')

const invalid = [-1, 0.5, NaN, Infinity, 2 ** 32, Number.MAX_SAFE_INTEGER, '1', true, {}]
for (const render of [toHtmlWithReport, toMarkdownWithReport, toPlainTextWithReport, toAnsiWithReport, toCarveWithReport]) {
  for (const maximum of invalid) assert.throws(() => render('body', false, maximum), TypeError)
  for (const strict of ['false', 0, 1, {}]) assert.throws(() => render('body', strict), TypeError)
  assert.equal(render('body', null, null).totalLosses, 0)
}
const losses = toHtmlWithReport('`a`{=latex} `b`{=latex}', false, 1)
assert.equal(losses.losses.length, 1)
assert.equal(losses.totalLosses, 2)
assert.equal(losses.truncated, true)
assert.equal(toHtmlWithReport('`a`{=latex}', false, 0).losses.length, 0)
assert.equal(toHtmlWithReport('`a`{=latex}', false, 2 ** 32 - 1).losses.length, 1)

for (const field of ['maxDepth', 'maxBytes', 'maxResolverCalls', 'maxWarnings']) {
  for (const value of invalid) {
    assert.throws(() => expandIncludes('{{ child }}', { resolve: () => 'body', [field]: value }), TypeError, field)
  }
}
let calls = 0
const refused = expandIncludes('{{ child }}', { resolve: () => { calls++; return 'body' }, maxResolverCalls: 0 })
assert.equal(calls, 0)
assert.equal(refused.warnings[0].rule, 'include-call-limit')
console.log('wasm artifact: edit ranges, report inputs and include budgets are checked')
