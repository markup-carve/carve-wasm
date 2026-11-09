import assert from 'node:assert/strict'
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { spawnSync } from 'node:child_process'

const source = "````text\n::: compare\n```carve\nfake\n```\n```html\nfake\n```\n:::\n````\n::: compare no-render\n````carve\n::: compare\n```html\nliteral\n```\n:::\n````\n```html\n<p>first</p>\n```\n```carve\nsecond\n```\n```html\n<p>second</p>\n```\n:::\n"
const root = mkdtempSync(join(tmpdir(), 'carve-population-'))
try {
  const examples = join(root, 'resources', 'examples')
  const corpus = join(root, 'tests', 'corpus')
  mkdirSync(examples, { recursive: true })
  mkdirSync(corpus, { recursive: true })
  for (const page of ['core.md', 'extensions.md', 'edge-cases.md']) {
    writeFileSync(join(examples, page), page === 'core.md' ? source : '')
  }
  for (const name of ['first', 'second']) {
    writeFileSync(join(corpus, `${name}.crv`), name)
    writeFileSync(join(corpus, `${name}.html`), `<p>${name}</p>`)
  }
  const run = () => spawnSync(process.execPath, ['tests/corpus-source.mjs'], {
    encoding: 'utf8', env: { ...process.env, CARVE_SPEC_CORPUS: corpus },
  })
  let result = run()
  assert.equal(result.status, 0, result.stderr)
  rmSync(join(corpus, 'second.html'))
  result = run()
  assert.notEqual(result.status, 0, 'a truncated corpus must fail')
  assert.match(result.stderr, /corpus pairs under/)
  writeFileSync(join(corpus, 'second.html'), '<p>second</p>')
  for (const [invalid, message] of [
    ['::: compare\n:::', /unpaired or empty/],
    ['::: compare\n```carve\nx\n```\n:::', /unpaired or empty/],
    ['::: compare', /unclosed/],
  ]) {
    writeFileSync(join(examples, 'core.md'), invalid)
    result = run()
    assert.notEqual(result.status, 0, 'invalid source must fail')
    assert.match(result.stderr, message)
  }
} finally {
  rmSync(root, { recursive: true, force: true })
}
console.log('corpus population: multi-pair sources, literal fences and refusals pass')
