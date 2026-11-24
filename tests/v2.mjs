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
const parts = { 'part.crv': 'Included *text*.\n', 'loop.crv': '{{ loop.crv }}\n' }
const resolver = ({ path }) => (Object.hasOwn(parts, path) ? { text: parts[path], id: path } : { error: 'not_found' })

// C2-N-003, C2-N-007: the host resolves includes; the engine applies its own rules.
test('includes expand through a host resolver', () => {
  assert.equal(wasm.render('{{ part.crv }}\n', { includes: resolver }), '<p>Included <strong>text</strong>.</p>')
  assert.equal(wasm.render('{{ part.crv }}\n', { includes: { resolver, file: 'main.crv' } }), '<p>Included <strong>text</strong>.</p>')
  const tree = wasm.parse('{{ part.crv }}\n', { includes: resolver })
  assert.equal(tree.children[0].type, 'include')
  assert.equal(tree.children[0].file, 'part.crv')
  const bytes = { resolver: () => ({ text: new TextEncoder().encode('bytes'), id: 'b.crv' }) }
  assert.equal(wasm.render('{{ b.crv }}\n', { includes: bytes }), '<p>bytes</p>')
})
test('host resolver errors, throws and cycles become include diagnostics', () => {
  const codes = (source, includes) => wasm.renderWithDiagnostics(source, { includes }).diagnostics.map((d) => d.code)
  assert.ok(codes('{{ gone.crv }}\n', resolver).includes('include-not-found'))
  assert.ok(codes('{{ x.crv }}\n', () => { throw new Error('boom') }).includes('include-unreadable'))
  assert.ok(codes('{{ loop.crv }}\n', resolver).some((code) => code.startsWith('include-')))
})
test('a host include config refuses unknown fields and bad limits', () => {
  assert.throws(() => wasm.render('x', { includes: { resolver, nope: 1 } }), TypeError)
  assert.throws(() => wasm.render('x', { includes: { resolver, depth: -1 } }), TypeError)
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

test('unknown writer options fail clearly', () => {
  const tree = wasm.parse('Hello')
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
  for (const name of wasm.extensions()) wasm.render('Hi', { extensions: [name] })
  assert.ok(wasm.render('::: details\nBody\n:::\n', { extensions: ['carve.details'], renderers: { 'carve.details': 'standard' } }).includes('<details>'))
  assert.equal(wasm.render(':wiki[Page]', { extensions: ['carve.wiki'] }), '<p>Page</p>')
  const source = '::: tabs\n::: tab {title=A}\nx\n:::\n:::\n'
  const html = wasm.render(source, { extensions: ['carve.tabs'], renderers: { 'carve.tabs': 'standard' } })
  assert.ok(html.includes('class="tabs"'))
  assert.throws(() => wasm.render('x', { extensions: ['math-block'] }), /extensions/)
  assert.throws(() => wasm.render('x', { renderers: { 'carve.tabs': 'host' } }), /standard/)
})

test('navigation extensions are listed and selectable', () => {
  for (const name of ["carve.autolink", "carve.externalLinks", "carve.headingNumbers", "carve.headingPermalinks", "carve.headingReference", "carve.linkPolicy", "carve.toc", "carve.tocPlacement"]) {
    assert.ok(wasm.extensions().includes(name), name)
    assert.equal(wasm.render('Hi', { extensions: [name] }), '<p>Hi</p>')
  }
})

test('C2-W-001: Markdown writer is registered', () => {
  assert.equal(wasm.toMarkdown('Hello'), 'Hello\n')
  assert.equal(wasm.toMarkdownWithOptions('Hello', {}), 'Hello\n')
})
test('C2-W-001: plain writer is registered', () => {
  assert.equal(wasm.toPlainText('Hello'), 'Hello\n')
  assert.equal(wasm.toPlainTextWithOptions('Hello', {}), 'Hello\n')
})
test('C2-W-001: ANSI writer is registered', () => {
  assert.equal(wasm.toAnsi('Hello'), 'Hello\n')
  assert.equal(wasm.toAnsiWithOptions('Hello', {}), 'Hello\n')
})
test('C2-A-003: parseWithLayout returns a layout sidecar', () => {
  const result = wasm.parseWithLayout('Hello')
  assert.equal(result.tree.type, 'document')
  assert.equal(result.layout.source, 'Hello')
  assert.equal(result.layout.version, 2)
})
test('C2-A-178: migrateTree returns a 0.2 tree', () => {
  const result = wasm.migrateTree({ type: 'document', children: [] })
  assert.equal(result.tree.astVersion, '2.0')
  assert.deepEqual(result.tree.children, [])
})
