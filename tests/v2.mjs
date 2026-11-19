import test from 'node:test'
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const wasm = require('../pkg/node/carve_wasm.js')

// C2-T-005 and C2-T-040: the binding keeps the engine's call and error shapes.
test('JSON boundary parses, renders, and reports typed errors', () => {
  const rendered = JSON.parse(wasm.call(JSON.stringify({ call: 'render', source: 'a *b*' })))
  assert.equal(rendered.ok.html, '<p>a <strong>b</strong></p>')
  const refused = JSON.parse(wasm.call(JSON.stringify({ call: 'render', source: 'x', options: { unknown: true } })))
  assert.equal(refused.error.name, 'CarveOptionError')
  assert.equal(refused.error.category, 'usage')
})

test('C2-T-005: convenience calls cover parse, render, lint and tree JSON', () => {
  const tree = wasm.parse('# Title')
  assert.equal(tree.type, 'document')
  assert.match(wasm.render('# Title'), /<h1>Title<\/h1>/)
  assert.equal(wasm.toHtml('a *b*'), '<p>a <strong>b</strong></p>')
  assert.equal(wasm.renderTree(tree), wasm.render('# Title'))
  assert.deepEqual(wasm.decodeTree(wasm.encodeTree(tree)), tree)
  assert.ok(Array.isArray(wasm.lint('Hello')))
  assert.ok(Array.isArray(wasm.includeDependencies(tree)))
  assert.ok(Array.isArray(wasm.parseWithDiagnostics('Hello').diagnostics))
  assert.ok(Array.isArray(wasm.renderWithDiagnostics('Hello').diagnostics))
})

test('C2-H-127: raw HTML stays off by default and is an explicit option', () => {
  const source = '```=html\n<script>alert(1)</script>\n```'
  assert.ok(!wasm.render(source).includes('<script>'))
  assert.ok(wasm.render(source, { rawHtml: true }).includes('<script>'))
  assert.throws(() => wasm.render('text', { rawHtml: 'false' }), /rawHtml/)
})

test('C2-T-005: HTML writer uses the registered target', () => {
  const tree = wasm.parse('Hello')
  assert.equal(wasm.write(tree, 'html'), wasm.render('Hello'))
})

test('source patches keep UTF-8 boundaries and stale-source refusal', () => {
  const source = 'a😀b'
  const patch = wasm.createSourcePatch(source, 'a😀c', 'quick-fix', 'change-last')
  assert.equal(wasm.applySourcePatch(source, patch), 'a😀c')
  assert.throws(() => wasm.applySourcePatch('stale', patch))
})

test('C2-W-001: source writer pending upstream issue 4327', { skip: 'carve writer not registered' }, () => {
  wasm.toCarve('Hello')
})
test('C2-W-001: Markdown, plain and ANSI writers pending upstream issue 4327', { skip: 'writer targets not registered' }, () => {
  wasm.toMarkdown('Hello'); wasm.toPlainText('Hello'); wasm.toAnsi('Hello')
})
test('C2-M-080: HTML importer pending upstream issue 4328', { skip: 'importDocument unsupported' }, () => {
  wasm.importDocument('<p>Hello</p>', 'html')
})
test('C2-X-052: extensions pending upstream issue 4329', { skip: 'extension host unsupported' }, () => {
  wasm.extensions()
})
