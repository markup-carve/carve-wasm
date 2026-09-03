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
  'toHtmlWithSymbols',
  'version',
]) {
  assert.equal(typeof wasm[name], 'function', `${name} should be exported`)
}
assert.match(wasm.toHtmlFull('# Render only'), /<h1>Render only/)

for (const name of [
  'htmlToCarve',
  'fromHtml',
  'fromMarkdown',
  'parseJson',
  'toHtmlWithReport',
  'toMarkdown',
  'toMarkdownWithReport',
  'toPlainText',
  'toPlainTextWithReport',
  'toAnsi',
  'toAnsiWithReport',
  'toCarve',
  'toCarveWithReport',
  'astJsonToHtml',
  'astJsonToCarve',
  'lintCarve',
  'readStamp',
  'needsReview',
  'fromDjot',
  'fromBbcode',
]) {
  assert.equal(wasm[name], undefined, `${name} should not be exported`)
}

console.log('rendering-only web artifact loads without HTML import exports')
