import test from 'node:test'
import assert from 'node:assert/strict'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const wasm = require('../pkg/node/carve_wasm.js')

const DOC = 'intro\n%% #region setup\none\ntwo\n%% #endregion\n// #region code\nx = 1\n// #endregion\n'

test('selectIncludeRegion returns the content lines', () => {
  assert.deepEqual(wasm.selectIncludeRegion(DOC, 'setup', 'carve'), { start: 3, end: 4 })
  assert.deepEqual(wasm.selectIncludeRegion(DOC, 'code', 'code'), { start: 7, end: 7 })
  assert.deepEqual(wasm.selectIncludeRegion('%% #region e\n%% #endregion\n', 'e', 'carve'), { start: 2, end: 1 })
  assert.deepEqual(wasm.selectIncludeRegion('\ufeff%% #region b\ny\n%% #endregion\n', 'b', 'carve'), { start: 2, end: 2 })
})

test('selectIncludeRegion throws IncludeRegionError for an invalid region', () => {
  for (const [source, name, mode] of [
    [DOC, 'code', 'carve'],
    [DOC, 'missing', 'code'],
    ['%% #region a\nx\n', 'a', 'carve'],
    ['%% #region a\n%% #endregion\n%% #region a\n%% #endregion\n', 'a', 'code'],
  ]) {
    assert.throws(() => wasm.selectIncludeRegion(source, name, mode),
      error => error.name === 'IncludeRegionError' && error.rule === 'include-region-invalid')
  }
  assert.throws(() => wasm.selectIncludeRegion(DOC, 'setup', 'markdown'), TypeError)
})

test('parseSnapshot and reparse return the tree parse writes', () => {
  const source = '# One\n\nText\n'
  const first = JSON.parse(wasm.parseSnapshot(source))
  assert.equal(first.source, source)
  assert.deepEqual(first.document, wasm.parse(source))
  assert.deepEqual(first.changedSource, [[0, Buffer.byteLength(source)]])
  const next = JSON.parse(wasm.reparse(source, '[{"range":[2,5],"replacement":"Two"}]'))
  assert.equal(next.source, '# Two\n\nText\n')
  assert.deepEqual(next.document, wasm.parse(next.source))
  assert.throws(() => wasm.reparse(source, '[{"range":[0,999],"replacement":""}]'), TypeError)
  assert.throws(() => wasm.reparse(source, '{}'), TypeError)
})

test('ParserSession keeps its snapshot and survives a rejected edit', () => {
  const source = 'First paragraph.\n\nSecond paragraph.\n\nThird paragraph.\n'
  const session = new wasm.ParserSession(source)
  try {
    const first = JSON.parse(session.snapshot())
    assert.equal(first.parsedSourceBytes, Buffer.byteLength(source))
    const next = JSON.parse(session.edit('[{"range":[0,5],"replacement":"Other"}]'))
    assert.equal(next.source, source.replace('First', 'Other'))
    assert.deepEqual(next.document, wasm.parse(next.source))
    assert.equal(next.reusedPreviousTree, true)
    assert.ok(next.parsedSourceBytes < Buffer.byteLength(next.source))
    const before = session.snapshot()
    assert.throws(() => session.edit('[{"range":[0,9999],"replacement":"X"}]'), TypeError)
    assert.equal(session.snapshot(), before)
    const syntax = JSON.parse(session.edit('[{"range":[0,0],"replacement":"= "}]'))
    assert.deepEqual(syntax.document, wasm.parse(syntax.source))
    assert.equal(syntax.reusedPreviousTree, false)
  } finally { session.free() }
})

const MERMAID = '```mermaid\ngraph TD;\n```\n'
function claim(tree) {
  tree.extensions = [{ name: 'ex.mermaid', version: '1', required: false, slots: [] }]
  tree.children = tree.children.map(node => node.type === 'code_block' && node.syntax === 'mermaid'
    ? { type: 'block_extension', name: 'ex.mermaid', version: '1', children: [], fallback: [node] }
    : node)
  return tree
}

test('toHtmlWithRenderers calls a host renderer on a claimed node', () => {
  const svg = node => `<svg>${node.fallback[0].value.trim()}</svg>`
  const result = wasm.toHtmlWithRenderers(MERMAID, {
    mode: 'static', transforms: [claim], renderers: { 'ex.mermaid': { static: svg } },
  })
  assert.equal(result.html, '<svg>graph TD;</svg>')
  assert.deepEqual(result.diagnostics, [])
  assert.equal(result.suppressedDiagnostics, 0)
  const interactive = wasm.toHtmlWithRenderers(MERMAID, {
    transforms: [claim], renderers: { 'ex.mermaid': { static: svg } },
  })
  assert.equal(interactive.html, wasm.render(MERMAID))
  const both = wasm.toHtmlWithRenderers(MERMAID, { transforms: [claim], renderers: { 'ex.mermaid': svg } })
  assert.equal(both.html, '<svg>graph TD;</svg>')
})

test('a failing renderer or transform keeps the fallback and reports it', () => {
  const boom = () => { throw new Error('callback failed') }
  const failed = wasm.toHtmlWithRenderers(MERMAID, { transforms: [claim], renderers: { 'ex.mermaid': boom } })
  assert.equal(failed.html, wasm.render(MERMAID))
  assert.deepEqual(failed.diagnostics.map(d => d.code), ['extension-renderer-failed'])
  const notText = wasm.toHtmlWithRenderers(MERMAID, { transforms: [claim], renderers: { 'ex.mermaid': () => 42 } })
  assert.deepEqual(notText.diagnostics.map(d => d.code), ['extension-renderer-failed'])
  const transform = wasm.toHtmlWithRenderers('= Guide\n', { transforms: [boom] })
  assert.equal(transform.html, wasm.render('= Guide\n'))
  assert.deepEqual(transform.diagnostics.map(d => d.code), ['extension-transform-failed'])
})

test('toHtmlWithRenderers keeps render options and standard renderers', () => {
  assert.equal(wasm.toHtmlWithRenderers('Ship :rocket:', { symbols: { rocket: '🚀' } }).html,
    wasm.render('Ship :rocket:', { symbols: { rocket: '🚀' } }))
  const tabs = '::: tabs\n::: tab {title=A}\nx\n:::\n:::\n'
  for (const selection of ['standard', { standard: { mode: 'aria' } }]) {
    const options = { extensions: ['carve.tabs'], renderers: { 'carve.tabs': selection } }
    assert.equal(wasm.toHtmlWithRenderers(tabs, options).html, wasm.render(tabs, options))
    assert.equal(wasm.toHtmlWithRenderers(tabs, { ...options, transforms: [tree => tree] }).html,
      wasm.render(tabs, options))
  }
})

test('toHtmlWithRenderers refuses host code for a standard extension', () => {
  assert.throws(() => wasm.toHtmlWithRenderers('Hi', { extensions: ['carve.tabs'], renderers: { 'carve.tabs': () => '' } }),
    error => error.name === 'CarveOptionError' && /C2-T-010/.test(error.message))
  assert.throws(() => wasm.toHtmlWithRenderers('Hi', { renderers: { 'carve.tabs': 'standard' } }),
    error => error.name === 'CarveOptionError')
  for (const renderers of [{ 'carve.tabs': { standard: { mode: 'bogus' } } }, { 'carve.details': { standard: { mode: 'aria' } } }]) {
    const options = { extensions: Object.keys(renderers), renderers }
    assert.throws(() => wasm.render('Hi', options), error => error.name === 'CarveOptionError')
    assert.throws(() => wasm.toHtmlWithRenderers('Hi', options), error => error.name === 'CarveOptionError')
  }
  const group = '::: code-group\n``` js [a.js]\nx\n```\n:::\n'
  const aria = { extensions: ['carve.codeGroup'], renderers: { 'carve.codeGroup': { standard: { mode: 'aria' } } } }
  assert.equal(wasm.toHtmlWithRenderers(group, aria).html, wasm.render(group, aria))
  assert.throws(() => wasm.toHtmlWithRenderers('Hi', { transforms: ['x'] }), error => error.name === 'CarveOptionError')
  assert.throws(() => wasm.toHtmlWithRenderers('Hi', { unknown: true }), error => error.name === 'CarveOptionError')
})
