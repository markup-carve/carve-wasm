import assert from 'node:assert/strict'
import { toCarve, toHtml } from './engine.mjs'

for (const marker of ['/', '!', '_']) {
  const source = `- [${marker}] task\n`
  assert.match(toHtml(source), new RegExp(`data-task-state="${marker}"`))
  assert.equal(toCarve(source), source)
}
