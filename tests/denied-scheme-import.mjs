import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { htmlToCarve, htmlToAst, astJsonToCarve } from './engine.mjs'

const fixture = (name) => readFileSync(new URL(`./fixtures/denied-scheme-destination/${name}`, import.meta.url), 'utf8')
const input = fixture('input.html')
const expected = fixture('expected.crv')
const expectedAst = JSON.parse(fixture('expected.ast.json'))
const report = JSON.parse(fixture('expected.report.json'))

const rows = (result) => result.report.diagnostics.map(({ code, message, severity, fidelity, confidence }) =>
  ({ code, message, severity, fidelity, confidence }))

// Every mode, `roundtrip` included: no Carve renderer writes such a destination, so
// there is nothing for the trusted mode to recover (docs/html-import-contract.md,
// "A denied destination is not a destination either").
for (const mode of ['safe', 'semantic', 'roundtrip']) {
  for (const [name, result, value] of [
    ['htmlToCarve', htmlToCarve(input, mode), (value) => value],
    ['htmlToAst', htmlToAst(input, mode), astJsonToCarve],
  ]) {
    assert.equal(value(result.value), expected, `${name}/${mode}: denied destinations must become text`)
    assert.equal(result.report.adapter, report.adapter)
    assert.deepEqual(rows(result), report.diagnostics, `${name}/${mode}: report each removed destination`)
  }
}
assert.equal(htmlToCarve(input, 'safe').report.mode, report.mode)

// The tree half of the same contract, against the spec's own oracle.
const tree = JSON.parse(htmlToAst(input, 'safe').value)
delete tree.srcByteLength
assert.deepEqual(tree, expectedAst, 'htmlToAst: the tree matches the spec fixture')

// Every nesting level the walk has to reach, and a table cell, which is the one
// inline slot that is not reached through a block's `children`.
for (const [where, html, carve] of [
  ['blockquote/list/emphasis', '<blockquote><ul><li><p><em><a href="vbscript:x">deep</a></em></p></li></ul></blockquote>', '> {loose}\n> - /deep/\n'],
  ['table cell', '<table><tr><td><a href="ms-msdt:x">cell</a></td></tr></table>', '| cell |\n'],
]) {
  const result = htmlToCarve(html, 'safe')
  assert.equal(result.value, carve, `${where}: a nested denied destination is removed`)
  assert.deepEqual(rows(result), [report.diagnostics[0]], `${where}: one row per removal`)
}

// The contract's own equivalence, which needs no hand-written expectation: a denied
// destination "is imported exactly like an empty one". The engine already implements the
// empty half, so it arbitrates the denied half against every attribute shape.
for (const [where, denied, blank] of [
  ['title', '<a href="javascript:x" title="tip">hello</a>', '<a href="" title="tip">hello</a>'],
  ['id and title', '<a href="javascript:x" id="k" title="tip">hello</a>', '<a href="" id="k" title="tip">hello</a>'],
  ['class', '<a href="data:x" class="c">hello</a>', '<a href="" class="c">hello</a>'],
  ['image title', '<img src="data:x" alt="logo" title="tip">', '<img src="" alt="logo" title="tip">'],
  ['image, empty alt', '<img src="file:x" alt="">', '<img src="" alt="">'],
  ['image in a paragraph', '<p><img src="data:x" alt="logo" title="tip"> tail</p>', '<p><img src="" alt="logo" title="tip"> tail</p>'],
  ['nested', '<blockquote><ul><li><p><em><a href="vbscript:x">deep</a></em></p></li></ul></blockquote>', '<blockquote><ul><li><p><em><a href="">deep</a></em></p></li></ul></blockquote>'],
  ['table cell', '<table><tr><td><a href="ms-msdt:x">cell</a></td></tr></table>', '<table><tr><td><a href="">cell</a></td></tr></table>'],
  // The writer has already spelled the split scheme `Java%09Script:` by the time the
  // guard reads it, so this shape is what proves the probe resolves the escape.
  ['split scheme', '<a href="Java&#9;Script:x">hello</a>', '<a href="">hello</a>'],
  ['split scheme on an image', '<img src="da&#9;ta:x" alt="logo">', '<img src="" alt="logo">'],
  // An empty code span is dropped by the WRITER and kept by the tree, so a guard that
  // re-imported instead of reading the written source refuses this document.
  ['empty code span beside it', '<p>a<code></code>b <a href="javascript:x">x</a></p>', '<p>a<code></code>b <a href="">x</a></p>'],
]) {
  assert.equal(htmlToCarve(denied, 'safe').value, htmlToCarve(blank, 'safe').value,
    `${where}: a denied destination imports exactly like an empty one`)
}

// A figure is the shape the removed wrapper handled differently from the engine, so it
// gets the equivalence spelled out rather than a value: both halves unwrap, and only the
// denied one reports.
const figure = htmlToCarve('<figure><img src="data:x" alt="logo"><figcaption>cap</figcaption></figure>', 'safe')
const blankFigure = htmlToCarve('<figure><img src="" alt="logo"><figcaption>cap</figcaption></figure>', 'safe')
assert.equal(figure.value, blankFigure.value, 'figure: a denied source imports exactly like an empty one')
assert.deepEqual(rows(figure).filter((row) => row.code === 'attribute-dropped'), [report.diagnostics[2]],
  'figure: one row for the removed source')

// The control: an allowed destination keeps its link and reports nothing, so the guard
// cannot pass by refusing every URL.
const allowed = '<p><a href="https://example.com">ok</a> <a href="#frag">frag</a> <a href="mailto:a@b.c">mail</a></p>\n<img src="pic.png" alt="pic">'
for (const mode of ['safe', 'roundtrip']) {
  const result = htmlToCarve(allowed, mode)
  assert.equal(result.value, '[ok](https://example.com) [frag](#frag) [mail](mailto:a@b.c)\n\n![pic](pic.png)\n',
    `allowed/${mode}: an allowed destination survives`)
  assert.deepEqual(result.report.diagnostics, [], `allowed/${mode}: nothing to report`)
}

// An escape the AUTHOR wrote is part of a relative name, and no consumer reads it as a
// scheme, so it survives: the writer's own escaping is resolved, a literal one is not.
for (const [name, html, carve] of [
  ['encoded colon', '<a href="javascript%3Ax">file</a>', '[file](javascript%3Ax)\n'],
  ['encoded colon on an image', '<img src="data%3Ax" alt="pic">', '![pic](data%3Ax)\n'],
  ['encoded tab', '<a href="java%09script:x">file</a>', '[file](java%09script:x)\n'],
  ['encoded tab on an image', '<img src="da%09ta:x" alt="pic">', '![pic](da%09ta:x)\n'],
  ['encoded tab beside a real one', '<p><a href="java%09script:x">keep</a> <a href="Java&#9;Script:y">drop</a></p>',
    '[keep](java%09script:x) drop\n'],
]) {
  const result = htmlToCarve(html, 'safe')
  assert.equal(result.value, carve, `${name}: an author's own escape is not a scheme`)
  for (const [where, value] of [['htmlToCarve', htmlToCarve(html, 'safe')], ['htmlToAst', htmlToAst(html, 'safe')]]) {
    assert.equal(value.report.diagnostics.filter((d) => d.code === 'attribute-dropped').length,
      name.endsWith('a real one') ? 1 : 0, `${name}/${where}: only a real denied scheme is reported`)
  }
}

console.log('wasm artifact: HTML import drops and reports denied destinations in every mode')
