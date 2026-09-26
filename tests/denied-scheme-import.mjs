import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { htmlToCarve, htmlToAst, astJsonToCarve } from './engine.mjs'

const fixture = (name) => readFileSync(new URL(`./fixtures/denied-scheme-destination/${name}`, import.meta.url), 'utf8')
const input = fixture('input.html')
const expected = fixture('expected.crv')
const report = JSON.parse(fixture('expected.report.json'))

for (const [name, result, value] of [
  ['htmlToCarve', htmlToCarve(input, 'safe'), (value) => value],
  ['htmlToAst', htmlToAst(input, 'safe'), astJsonToCarve],
]) {
  assert.equal(value(result.value), expected, `${name}: denied destinations must become text`)
  assert.equal(result.report.mode, report.mode)
  assert.equal(result.report.adapter, report.adapter)
  assert.deepEqual(result.report.diagnostics.map(({ code, message, severity, fidelity, confidence }) =>
    ({ code, message, severity, fidelity, confidence })), report.diagnostics, `${name}: report each removed destination`)
}
console.log('wasm artifact: safe import drops and reports denied destinations')
