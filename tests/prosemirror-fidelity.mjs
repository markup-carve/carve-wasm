import assert from 'node:assert/strict'
import { toProseMirror, fromProseMirror, toCarve } from './engine.mjs'
const cases = ['*before*\n', 'a [x](https://example.com)\n', '# Heading\n\nbody\n', '*[HTML]: Hypertext\n\nHTML\n', ':: term\n: [label]: https://example.com\n', '- one\n  - two\n']
for (const source of cases) {
  const result = toProseMirror(source)
  const changed = toCarve(fromProseMirror(result.json)) !== toCarve(source)
  if (changed) assert.ok(Object.keys(result.dropped).length + Object.keys(result.degraded).length > 0, 'the bridge silently changes canonical source')
}
console.log('wasm artifact: the ProseMirror bridge reports canonical source changes')
