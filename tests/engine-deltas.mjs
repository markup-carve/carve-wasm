// What the engine bump to carve-lang 0.1.8 changed where this package can see
// it. Each case was measured against the previous pin as well, so a line here
// is a difference the artifact actually carries rather than a restatement of
// the engine's changelog.
import assert from 'node:assert/strict'
import { expandIncludes, lintCarve, parseJson, toCarve, toHtml } from './engine.mjs'

// `|{.r}`, five bytes, panicked the table check (carve-rs#2341). A panic
// reaches a wasm caller as `unreachable`, which is indistinguishable from an
// invalid document, so every entry point that parses is pinned.
//
// Not only the five bytes. A shape differential over a pipe followed by one
// attribute block, in each container that can hold it, found 36 panicking
// shapes on the previous pin out of 84 probed - so the cases below are the
// boundary, not a single input.
const carriers = [
  (line) => line,
  (line) => line + '\n',
  (line) => 'a\n\n' + line + '\n',
  (line) => '- ' + line,
  (line) => '> ' + line,
  (line) => ':::x\n' + line + '\n:::',
  (line) => '| a |\n' + line,
  (line) => '|a|\n|---|\n' + line,
  (line) => '|a|\n|---|\n|1|\n' + line,
  (line) => '| a | b |\n|---|---|\n| 1 | 2 |\n' + line,
]
for (const attrs of ['{.r}', '{.r .s}', '{#i}', '{ .r }']) {
  for (const carry of carriers) {
    const source = carry('|' + attrs)
    for (const [name, run] of [['toHtml', toHtml], ['toCarve', toCarve], ['parseJson', parseJson], ['lintCarve', lintCarve]]) {
      assert.ok(run(source) !== undefined, `${name} answered ${JSON.stringify(source)}`)
    }
  }
}
// The bare form is paragraph text, not a table.
assert.match(toHtml('|{.r}'), /\{\.r\}/)

// A reference image with no definition reports under the rule that already
// covered reference links, with no new id (carve-rs#2336). The previous pin
// reported nothing at all for the image.
assert.deepEqual(
  lintCarve('![alt][missing]\n').map(({ rule }) => rule),
  ['unresolved-reference-link'],
)
assert.deepEqual(
  lintCarve('[text][missing]\n').map(({ rule }) => rule),
  ['unresolved-reference-link'],
)

// `broken-fragment-link` is new to a consumer: engine 0.1.7 had no such id.
// The case-only near miss has to name the real id, which is the part a host
// turns into a quick fix.
const fragment = (source) => lintCarve(source).map(({ rule, message }) => ({ rule, message }))
assert.deepEqual(fragment('[t](#top)\n'), [], 'a bare #top link is not reported')
const [missing] = fragment('{#real}\n# Real\n\n[t](#nope)\n')
assert.equal(missing.rule, 'broken-fragment-link')
assert.match(missing.message, /"#nope" matches no id in this document/)
const [nearMiss] = fragment('{#real}\n# Real\n\n[t](#Real)\n')
assert.equal(nearMiss.rule, 'broken-fragment-link')
assert.match(nearMiss.message, /the id "real" differs only in case/)

// An include selects a block by its explicit id, not only a heading section
// (carve-rs#2311, carve#2727). The previous pin warned `include-section` here.
const included = '{#sec}\n# S\n\nbody\n\n{#para}\nlone para\n'
const resolve = () => included
const byId = expandIncludes('{{ other.crv #para }}\n', { resolve, sourcePath: 'main.crv' })
assert.deepEqual(byId.warnings, [], 'an explicit id on a paragraph resolves')
const tree = JSON.parse(byId.json)
assert.equal(tree.children.length, 1)
assert.equal(tree.children[0].type, 'paragraph')
assert.equal(tree.children[0].attrs.id, 'para')

// A section that matches nothing still warns, so the selector did not become
// permissive on the way.
const unknown = expandIncludes('{{ other.crv #nope }}\n', { resolve, sourcePath: 'main.crv' })
assert.deepEqual(unknown.warnings.map(({ rule }) => rule), ['include-section'])

console.log('wasm artifact: carve-lang 0.1.8 delta cases pass')
