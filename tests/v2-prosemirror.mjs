import test from 'node:test'
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const wasm = require('../pkg/node/carve_wasm.js')

const SOURCE = '== Notes\n\nSome *bold* text and [a link](https://example.com).\n\n- one\n- two\n'

test('toProseMirror and proseMirrorToCarve round-trip source', () => {
  const result = wasm.toProseMirror(SOURCE)
  assert.equal(result.doc.type, 'doc')
  assert.deepEqual(JSON.parse(result.json), result.doc)
  assert.deepEqual(result.blanked, [])
  assert.equal(result.doc.content[0].type, 'heading')
  assert.equal(result.doc.content[0].attrs.level, 2)
  assert.equal(wasm.proseMirrorToCarve(result.doc), SOURCE)
  assert.equal(wasm.proseMirrorToCarve(result.json), SOURCE)
})

test('toProseMirror takes a tree and fromProseMirror gives it back', () => {
  const tree = wasm.parse(SOURCE)
  const { doc } = wasm.toProseMirror(tree)
  assert.deepEqual(doc, wasm.toProseMirror(SOURCE).doc)
  const back = wasm.fromProseMirror(doc)
  assert.equal(back.type, 'document')
  assert.equal(wasm.write(back, 'carve'), SOURCE)
})

test('fromProseMirrorWithReport gives origins, pointers and ignored attributes', () => {
  const doc = {
    type: 'doc',
    content: [
      { type: 'paragraph', attrs: { textAlign: 'center' }, content: [{ type: 'text', text: 'x', marks: [{ type: 'italic' }] }] },
    ],
  }
  const report = wasm.fromProseMirrorWithReport(doc)
  assert.equal(report.tree.type, 'document')
  assert.deepEqual(report.origins, [null])
  assert.equal(report.pointers['/children/0'], '/content/0')
  assert.equal(report.pointers['/children/0/children/0'], '/content/0/content/0/marks/0')
  assert.deepEqual(report.ignored, [{ path: '/content/0', type: 'paragraph', attribute: 'textAlign', value: 'center' }])
  const loaded = wasm.fromProseMirrorWithReport(wasm.toProseMirror(SOURCE).doc)
  assert.deepEqual(loaded.origins, [0, 1, 2])
})

test('the options take a URL policy and a mapping', () => {
  const blanked = wasm.toProseMirror('[no](javascript:alert(1))', { urlPolicy: { allow: ['https'] } })
  assert.deepEqual(blanked.blanked, ['javascript:alert(1)'])
  const mapping = { nodes: { placeholderToken: { as: 'span', class: 'placeholder', attributes: ['name'] } } }
  const doc = { type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text: 'Dear ' }, { type: 'placeholderToken', attrs: { name: 'first' } }] }] }
  const source = wasm.proseMirrorToCarve(doc, { mapping })
  assert.equal(source, 'Dear []{.placeholder name=first}\n')
  assert.deepEqual(wasm.toProseMirror(source, { mapping }).doc.content[0].content, doc.content[0].content)
  assert.throws(() => wasm.toProseMirror(SOURCE, { profile: 'comment' }), /not an option/)
})

test('a node with no Carve node throws', () => {
  assert.throws(
    () => wasm.fromProseMirror({ type: 'doc', content: [{ type: 'mention', attrs: { id: '7' } }] }),
    error => error instanceof Error && error.name === 'ProseMirrorError' && /mention/.test(error.message),
  )
  assert.throws(() => wasm.proseMirrorToCarve('{'), Error)
  assert.throws(
    () => wasm.fromProseMirror({ type: 'doc', content: [{ type: 'carveBlock', attrs: { carve: { type: 'nope' } } }] }),
    error => error.name === 'CarveTreeError' && error.category === 'ast-json-invalid' && typeof error.status === 'number',
  )
})
