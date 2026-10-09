import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { fromMarkdown, htmlToCarve, toCarveWithReport, toHtml, toHtmlWithOptions } from './engine.mjs'

const cases = JSON.parse(readFileSync(new URL('./fixtures/ordered-dialect-boundaries.json', import.meta.url), 'utf8'))
assert.equal(cases.length, 17)
for (const row of cases) {
  assert.equal(toHtml(row.source).replace(/\n+$/, ''), row.html, row.name)
  const formatted = toCarveWithReport(row.source, true)
  assert.equal(formatted.value, row.source, row.name)
  assert.deepEqual(formatted.losses, [], row.name)
  assert.equal(formatted.totalLosses, 0, row.name)
  const imported = htmlToCarve(row.inputHtml, 'safe')
  assert.equal(imported.value, row.source, row.name)
  assert.deepEqual(imported.report.diagnostics, [], row.name)
  assert.equal(toHtml(imported.value).replace(/\n+$/, ''), row.html, row.name)
}
assert.equal(toHtmlWithOptions('``` =html\n<b>x</b>\n```', { profile: 'article' }),
  '<pre><code class="language-html">&lt;b&gt;x&lt;/b&gt;\n</code></pre>')
const markdownCases = JSON.parse(readFileSync(new URL('./fixtures/markdown-ordered-delimiters.json', import.meta.url), 'utf8'))
assert.equal(markdownCases.length, 3)
for (const row of markdownCases) {
  const imported = fromMarkdown(row.markdown)
  assert.equal(imported.value, row.source, row.name)
  assert.equal(toHtml(imported.value).replace(/\n+$/, ''), row.html, row.name)
  assert.ok(imported.report.diagnostics.length > 0, row.name)
  assert.ok(imported.report.diagnostics.every(diagnostic => diagnostic.fidelity === 'preserved'), row.name)
}
console.log('ordered dialect boundaries: 17 HTML cases, three Markdown cases and article raw payload pass')
