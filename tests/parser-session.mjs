import assert from 'node:assert/strict'
import { ParserSession, parseJson } from './engine.mjs'
const source = 'First paragraph.\n\nSecond paragraph.\n\nThird paragraph.\n'
const session = new ParserSession(source)
try {
  const first = JSON.parse(session.snapshot())
  assert.equal(first.parsedSourceBytes, Buffer.byteLength(source))
  const next = JSON.parse(session.edit('[{"range":[0,5],"replacement":"Other"}]'))
  assert.equal(next.source, source.replace('First', 'Other'))
  assert.deepEqual(next.document, JSON.parse(parseJson(next.source)))
  assert.equal(next.reusedPreviousTree, true)
  assert.ok(next.parsedSourceBytes < Buffer.byteLength(next.source))
  const before = session.snapshot()
  assert.throws(() => session.edit('[{"range":[0,9999],"replacement":"X"}]'), TypeError)
  assert.equal(session.snapshot(), before)
  assert.equal(JSON.parse(session.edit('[]')).parsedSourceBytes, 0)
  const syntax = JSON.parse(session.edit('[{"range":[0,0],"replacement":"# "}]'))
  assert.deepEqual(syntax.document, JSON.parse(parseJson(syntax.source)))
  assert.equal(syntax.reusedPreviousTree, false)
} finally { session.free() }
console.log('wasm artifact: parser sessions retain snapshots and preserve state after invalid edits')
