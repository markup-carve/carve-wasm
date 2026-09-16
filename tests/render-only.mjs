import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'
import { pathToFileURL } from 'node:url'
import path from 'node:path'

const packageDirectory = process.env.CARVE_WASM_PKG
if (!packageDirectory) {
  throw new Error('CARVE_WASM_PKG must name the rendering-only wasm-pack output')
}

const modulePath = path.resolve(packageDirectory, 'carve_wasm.js')
const wasm = await import(pathToFileURL(modulePath))

assert.equal(typeof wasm.default, 'function')
await wasm.default({
  module_or_path: await readFile(path.resolve(packageDirectory, 'carve_wasm_bg.wasm')),
})

for (const name of [
  'extensions',
  'toHtml',
  'toHtmlFull',
  'toHtmlWithOptions',
  'toHtmlWithRenderers',
  'toHtmlWithSymbols',
  'sanitizeSvg',
  'parseLocator',
  'version',
]) {
  assert.equal(typeof wasm[name], 'function', `${name} should be exported`)
}
assert.match(wasm.toHtmlFull('# Render only'), /<h1>Render only/)

// The static math renderer ships in the rendering-only selection too: a
// Playground that renders `mode: 'static'` is exactly the host that needs it.
const mathOnly = wasm.toHtmlWithRenderers('``` math\nE = mc^2\n```\n', {
  mode: 'static',
  extensions: ['math-block'],
  renderers: { math: (tex) => `<math>${tex}</math>` },
})
assert.equal(mathOnly.html, '<div class="math display"><math>E = mc^2</math></div>')
assert.deepEqual(mathOnly.rendererErrors, [])

for (const name of [
  'htmlToCarve',
  'fromHtml',
  'fromMarkdown',
  'parseJson',
  'parseJsonWithOptions',
  'parseSourceLayoutJson',
  'markdownToAstJson',
  'toHtmlWithReport',
  'toMarkdown',
  'toMarkdownWithOptions',
  'toMarkdownWithReport',
  'toPlainText',
  'toPlainTextWithOptions',
  'toPlainTextWithReport',
  'toAnsi',
  'toAnsiWithOptions',
  'toAnsiWithReport',
  'toCarve',
  'toCarveWithOptions',
  'toCarveWithReport',
  'astJsonToHtml',
  'astJsonToCarve',
  'applyProfile',
  'toProseMirror',
  'fromProseMirror',
  'lintCarve',
  'lintCarveWithOptions',
  'lintAccessibility',
  'readStamp',
  'stampCarve',
  'needsReview',
  'fromDjot',
  'fromBbcode',
  'migrateDjot',
  'migrateBbcode',
  'createSourcePatch',
  'toCarvePatch',
  'applySourcePatch',
]) {
  assert.equal(wasm[name], undefined, `${name} should not be exported`)
}

console.log('rendering-only web artifact loads without HTML import exports')
