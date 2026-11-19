import test from 'node:test'
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const wasm = require('../pkg/node/carve_wasm.js')
const title = '# Title'

// C2-T-005 and C2-T-040: the binding keeps the engine's call and error shapes.
test('call preserves the 0.2 JSON response', () => {
  const rendered = JSON.parse(wasm.call(JSON.stringify({ call: 'render', source: 'a *b*' })))
  assert.equal(rendered.ok.html, '<p>a <strong>b</strong></p>')
  const refused = JSON.parse(wasm.call(JSON.stringify({ call: 'render', source: 'x', options: { unknown: true } })))
  assert.equal(refused.error.name, 'CarveOptionError')
  assert.equal(refused.error.category, 'usage')
})

test('C2-T-005 parse returns a 0.2 document tree', () => {
  assert.equal(wasm.parse(title).type, 'document')
})
test('C2-T-005 parseWithDiagnostics carries the parse record', () => {
  const result = wasm.parseWithDiagnostics(title)
  assert.equal(result.tree.type, 'document')
  assert.ok(Array.isArray(result.diagnostics))
  assert.equal(typeof result.suppressedDiagnostics, 'number')
})
test('C2-T-005 render and the HTML aliases return HTML', () => {
  assert.match(wasm.render(title), /<h1>Title<\/h1>/)
  assert.equal(wasm.toHtml('a *b*'), '<p>a <strong>b</strong></p>')
  assert.equal(wasm.toHtmlWithOptions('Hello', {}), wasm.render('Hello'))
})
test('C2-T-005 renderWithDiagnostics carries the render record', () => {
  const result = wasm.renderWithDiagnostics('Hello')
  assert.equal(result.html, wasm.render('Hello'))
  assert.ok(Array.isArray(result.diagnostics))
})
test('C2-T-005 lint returns diagnostic entries', () => {
  assert.ok(Array.isArray(wasm.lint('Hello')))
})
test('C2-T-005 renderTree accepts a 0.2 tree', () => {
  assert.equal(wasm.renderTree(wasm.parse(title)), wasm.render(title))
})
test('C2-T-005 encodeTree and decodeTree round-trip 0.2 JSON', () => {
  const tree = wasm.parse(title)
  assert.deepEqual(wasm.decodeTree(wasm.encodeTree(tree)), tree)
})
test('C2-A-012 encodeEnvelope and validateTree pass through the JSON boundary', () => {
  const tree = wasm.parse('Hello')
  const validated = JSON.parse(wasm.call(JSON.stringify({ call: 'validateTree', tree })))
  assert.deepEqual(validated.ok, {})
  const encoded = JSON.parse(wasm.call(JSON.stringify({ call: 'encodeEnvelope', tree })))
  assert.equal(JSON.parse(encoded.ok.json).astVersion, '2.0')
})
test('C2-T-022 includeDependencies returns a record', () => {
  assert.ok(Array.isArray(wasm.includeDependencies(wasm.parse('Hello'))))
})
test('C2-T-005 HTML writer uses the registered target', () => {
  const tree = wasm.parse('Hello')
  assert.equal(wasm.write(tree, 'html'), wasm.render('Hello'))
  assert.equal(wasm.writeWithDiagnostics(tree, 'html').output, wasm.render('Hello'))
})

test('C2-H-127: raw HTML stays off by default and is an explicit option', () => {
  const source = '```=html\n<script>alert(1)</script>\n```'
  assert.ok(!wasm.render(source).includes('<script>'))
  assert.ok(wasm.render(source, { rawHtml: true }).includes('<script>'))
  assert.throws(() => wasm.render('text', { rawHtml: 'false' }), /rawHtml/)
})

test('source patches keep UTF-8 boundaries and stale-source refusal', () => {
  const source = 'a😀b'
  const patch = wasm.createSourcePatch(source, 'a😀c', 'quick-fix', 'change-last')
  assert.equal(wasm.applySourcePatch(source, patch), 'a😀c')
  assert.throws(() => wasm.applySourcePatch('stale', patch))
})

test('pending engine capabilities fail with clear availability errors', () => {
  const tree = wasm.parse('Hello')
  assert.throws(() => wasm.toMarkdown('Hello'), /markdown writer not available yet/)
  assert.throws(() => wasm.toMarkdownWithOptions('Hello', {}), /markdown writer not available yet/)
  assert.throws(() => wasm.toPlainText('Hello'), /plain writer not available yet/)
  assert.throws(() => wasm.toPlainTextWithOptions('Hello', {}), /plain writer not available yet/)
  assert.throws(() => wasm.toAnsi('Hello'), /ansi writer not available yet/)
  assert.throws(() => wasm.toAnsiWithOptions('Hello', {}), /ansi writer not available yet/)
  assert.throws(() => wasm.parseWithLayout('Hello'), /parseWithLayout is not available/)
  assert.throws(() => wasm.migrateTree({ type: 'document', children: [] }), /migrateTree is not available/)
  assert.throws(() => wasm.write(tree, 'carve', { nope: true }), /unknown option/)
  assert.throws(() => wasm.format('Hello', { nope: true }), /unknown option/)
})

// C2-W-035: the canonical writer writes a heading with `=`.
test('C2-W-001 source writer and format', () => {
  assert.equal(wasm.toCarve('# Hi'), '= Hi\n')
  assert.equal(wasm.toCarveWithOptions('# Hi', {}), '= Hi\n')
  assert.equal(wasm.write(wasm.parse('# Hi'), 'carve'), '= Hi\n')
  assert.equal(wasm.formatWithDiagnostics('# Hi').source, '= Hi\n')
  assert.equal(wasm.toCarvePatch('# Hi').edits[0].replacement, '= Hi\n')
})

test('C2-T-005 importDocument imports HTML', () => {
  const imported = wasm.importDocument('<p>Hello <strong>world</strong></p>', 'html')
  assert.equal(imported.source, 'Hello *world*\n')
  assert.equal(imported.tree.type, 'document')
  assert.deepEqual(imported.diagnostics, [])
})

test('C2-X-039 standard extensions are selected by name', () => {
  assert.ok(wasm.extensions().includes('carve.tabs'))
  assert.equal(wasm.render(':wiki[Page]', { extensions: ['carve.wiki'] }), '<p>Page</p>')
  const source = '::: tabs\n::: tab {title=A}\nx\n:::\n:::\n'
  const html = wasm.render(source, { extensions: ['carve.tabs'], renderers: { 'carve.tabs': 'standard' } })
  assert.ok(html.includes('class="tabs"'))
  assert.throws(() => wasm.render('x', { extensions: ['math-block'] }), /extensions/)
  assert.throws(() => wasm.render('x', { renderers: { 'carve.tabs': 'host' } }), /standard/)
})

test('C2-W-001: Markdown writer pending upstream issues 4327 and 4349', { skip: 'markdown writer not registered' }, () => {
  wasm.toMarkdown('Hello')
})
test('C2-W-001: plain writer pending upstream issues 4327 and 4349', { skip: 'plain writer not registered' }, () => {
  wasm.toPlainText('Hello')
})
test('C2-W-001: ANSI writer pending upstream issues 4327 and 4349', { skip: 'ansi writer not registered' }, () => {
  wasm.toAnsi('Hello')
})
test('C2-A-003: parseWithLayout pending upstream issue 4334', { skip: 'layout sidecar unsupported' }, () => {
  wasm.parseWithLayout('Hello')
})
test('C2-A-178: migrateTree pending upstream issue 4335', { skip: 'tree migration unsupported' }, () => {
  wasm.migrateTree({ type: 'document', children: [] })
})
