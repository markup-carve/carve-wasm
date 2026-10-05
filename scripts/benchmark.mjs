import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { gzipSync } from 'node:zlib'
import { readFileSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { performance } from 'node:perf_hooks'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = fileURLToPath(new URL('..', import.meta.url))
const pkg = resolve(process.env.CARVE_WASM_PKG || join(root, 'pkg'))
const samples = Number(process.env.CARVE_BENCH_SAMPLES || 10)
assert.ok(Number.isInteger(samples) && samples >= 2 && samples <= 100)
const nodeEntry = join(pkg, 'node', 'carve_wasm.js')
const cold = JSON.parse(execFileSync(process.execPath, ['-e', `const start=performance.now(); const w=require(${JSON.stringify(nodeEntry)}); w.toHtml('body'); console.log(JSON.stringify({milliseconds:performance.now()-start,rssBytes:process.memoryUsage().rss}))`], { encoding: 'utf8' }))
const wasm = await import(pathToFileURL(join(pkg, 'carve_wasm_bg.js')))
const instance = new WebAssembly.Instance(new WebAssembly.Module(readFileSync(join(pkg, 'carve_wasm_bg.wasm'))), { './carve_wasm_bg.js': wasm })
wasm.__wbg_set_wasm(instance.exports)
instance.exports.__wbindgen_start()
const paragraphs = Array.from({ length: 1000 }, (_, n) => `Paragraph ${n} has some ordinary text.`).join('\n\n') + '\n'
const syntax = Array.from({ length: 250 }, (_, n) => `# Heading ${n}\n\nA *strong* paragraph with [a link](https://example.com).\n`).join('\n')
function time(run) {
  run()
  const times = []
  for (let n = 0; n < samples; n++) { const start = performance.now(); run(); times.push(performance.now() - start) }
  times.sort((a, b) => a - b)
  return { medianMs: times[Math.floor(times.length / 2)], maxMs: times.at(-1) }
}
const render = time(() => wasm.toHtmlWithOptions(syntax, { rawHtml: false }))
const bridge = time(() => wasm.toProseMirror(syntax))
const stateless = time(() => wasm.reparse(paragraphs, '[{"range":[0,9],"replacement":"paragraph"}]'))
const session = new wasm.ParserSession(paragraphs)
let latest
let lowercase = true
const retained = time(() => {
  const replacement = lowercase ? 'paragraph' : 'Paragraph'
  lowercase = !lowercase
  latest = JSON.parse(session.edit(JSON.stringify([{ range: [0, 9], replacement }])))
  assert.ok(latest.source.startsWith(replacement))
})
assert.equal(latest.reusedPreviousTree, true)
assert.ok(latest.parsedSourceBytes < Buffer.byteLength(paragraphs) / 10)
session.free()
const memory = []
for (let n = 0; n < 100; n++) {
  wasm.toHtml(syntax)
  const state = new wasm.ParserSession(paragraphs)
  state.edit('[{"range":[0,9],"replacement":"paragraph"}]')
  state.free()
  if (n % 10 === 0) memory.push(instance.exports.memory.buffer.byteLength)
}
assert.ok(memory.at(-1) - memory[Math.floor(memory.length / 2)] <= 4 * 1024 * 1024, 'repeated renders and freed sessions keep growing WASM memory')
const bytes = directory => {
  const raw = readFileSync(join(pkg, directory, 'carve_wasm_bg.wasm'))
  return { raw: raw.length, gzip: gzipSync(raw, { level: 9 }).length }
}
const result = { node: process.version, samples, cold, bytes: { full: bytes(''), render: bytes('render') }, render, bridge, edits: { stateless, retained, sourceBytes: Buffer.byteLength(paragraphs), parsedSourceBytes: latest.parsedSourceBytes }, memoryBytes: memory }
writeFileSync(join(root, 'benchmark-results.json'), JSON.stringify(result, null, 2) + '\n')
console.log(JSON.stringify(result, null, 2))
