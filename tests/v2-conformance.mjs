import test from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'

const file = process.env.CARVE_V2_EXAMPLES
if (!file) throw new Error('CARVE_V2_EXAMPLES must name the pinned engine examples.json')
const require = createRequire(import.meta.url)
const wasm = require('../pkg/node/carve_wasm.js')
const rows = JSON.parse(readFileSync(file, 'utf8')).examples.filter(row =>
  row.kind === 'render' && typeof row.expected?.html === 'string'
)

// The pinned Rust engine has one open render bug. The skipped row is never an
// expected output for the binding and becomes active when the engine pin moves.
const behind = new Set(['html-69']) // carve-rs issue 4333, C2-H-168
for (const row of rows) {
  test(`0.2 ${row.id}`, { skip: behind.has(row.id) ? 'carve-rs issue 4333' : false }, () => {
    assert.equal(wasm.render(row.input, row.options), row.expected.html)
  })
}
assert.equal(rows.length, 505, 'conformance population changed; review the engine pin')
assert.ok(rows.some(row => row.id === 'html-69'))
