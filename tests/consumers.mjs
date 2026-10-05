import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { createServer } from 'node:http'
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { chromium, firefox, webkit } from 'playwright'
import { build } from 'vite'

const root = fileURLToPath(new URL('..', import.meta.url))
const pkg = resolve(process.env.CARVE_WASM_PKG || join(root, 'pkg'))
const work = join(root, 'consumer-artifact')
rmSync(work, { recursive: true, force: true })
mkdirSync(work, { recursive: true })
const packed = JSON.parse(execFileSync('npm', ['pack', '--json', '--pack-destination', work], { cwd: pkg, encoding: 'utf8' }))[0]
writeFileSync(join(work, 'package.json'), JSON.stringify({ private: true, type: 'module' }))
execFileSync('npm', ['install', '--ignore-scripts', '--no-audit', '--no-fund', join(work, packed.filename)], { cwd: work, stdio: 'inherit' })
cpSync(join(root, 'tests/types-consumer.ts'), join(work, 'consumer.ts'))
writeFileSync(join(work, 'tsconfig.json'), JSON.stringify({ compilerOptions: { strict: true, noEmit: true, skipLibCheck: false, target: 'ES2022', lib: ['ES2022', 'DOM', 'ESNext.Disposable'], module: 'NodeNext', moduleResolution: 'NodeNext' }, files: ['consumer.ts'] }))
execFileSync(process.execPath, [join(root, 'node_modules/typescript/bin/tsc'), '-p', join(work, 'tsconfig.json')], { stdio: 'inherit' })
const declared = Object.fromEntries(['web', 'render'].map(entry => [entry, [...readFileSync(join(pkg, entry, 'carve_wasm.d.ts'), 'utf8').matchAll(/export (?:function|class) (\w+)/g)].map(match => match[1])]))
writeFileSync(join(work, 'node.mjs'), `import assert from 'node:assert/strict'; import {toHtml} from '@markup-carve/carve-wasm'; import {toHtml as explicit} from '@markup-carve/carve-wasm/node'; assert.match(toHtml('# Node'), /Node/); assert.equal(explicit('# Node'), toHtml('# Node')); for (const path of ['carve_wasm_bg.wasm','web/carve_wasm_bg.wasm','render/carve_wasm_bg.wasm']) assert.ok(import.meta.resolve('@markup-carve/carve-wasm/'+path).endsWith('.wasm'));`)
writeFileSync(join(work, 'node.cjs'), `const assert = require('node:assert/strict'); const {toHtml} = require('@markup-carve/carve-wasm'); assert.match(toHtml('# Node'), /Node/);`)
for (const file of ['node.mjs', 'node.cjs']) execFileSync(process.execPath, [join(work, file)], { stdio: 'inherit' })
writeFileSync(join(work, 'index.html'), '<!doctype html><script type="module" src="/main.js"></script>')
cpSync(join(root, 'examples/carve-worker.mjs'), join(work, 'carve-worker.mjs'))
writeFileSync(join(work, 'main.js'), `import * as bundled from '@markup-carve/carve-wasm'; import init, * as web from '@markup-carve/carve-wasm/web'; import renderInit, * as render from '@markup-carve/carve-wasm/render'; import {utf16ToUtf8} from '@markup-carve/carve-wasm/offsets'; await init(); await renderInit(); window.carve = {bundled, web, render, utf16ToUtf8}; window.worker = new Worker(new URL('./carve-worker.mjs', import.meta.url), {type:'module'});`)
await build({ root: work, logLevel: 'warn', build: { target: 'esnext' } })
const dist = join(work, 'dist')
const server = createServer((request, response) => {
  try {
    const url = new URL(request.url, 'http://localhost')
    const file = resolve(dist, '.' + (url.pathname === '/' ? '/index.html' : url.pathname))
    if (!file.startsWith(dist + '/')) { response.writeHead(403).end(); return }
    response.setHeader('Content-Type', file.endsWith('.wasm') ? 'application/wasm' : file.endsWith('.js') ? 'text/javascript' : 'text/html')
    response.end(readFileSync(file))
  } catch { response.writeHead(404).end() }
})
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve))
try {
  for (const [name, type] of Object.entries({ chromium, firefox, webkit })) {
    const executablePath = process.env[`CARVE_${name.toUpperCase()}_EXECUTABLE`]
    const browser = await type.launch({ headless: true, ...(executablePath ? { executablePath } : {}) })
    try {
      const page = await browser.newPage()
      const errors = []
      page.on('pageerror', error => errors.push(String(error)))
      await page.goto(`http://127.0.0.1:${server.address().port}/`)
      await page.waitForFunction(() => !!window.carve)
      const result = await page.evaluate(declared => {
        const { bundled, web, render, utf16ToUtf8 } = window.carve
        const source = ':::: note\n'.repeat(200) + 'deep\n' + '::::\n'.repeat(200)
        const engines = [bundled, web]
        return {
          html: engines.map(w => w.toHtml('# Browser')),
          deep: engines.map(w => JSON.parse(w.parseJson(source)).type),
          safe: [...engines, render].map(w => w.toHtmlWithOptions('``` =html\n<script>alert(1)</script>\n```', { rawHtml: false, profile: 'comment' })),
          rejected: engines.map(w => { try { w.reparse('abc', '[{"range":[4294967296,4294967297],"replacement":"X"}]'); return false } catch(e) { return e instanceof TypeError } }),
          missingExports: Object.entries({ web, render }).flatMap(([entry, module]) => declared[entry].filter(name => typeof module[name] !== 'function').map(name => entry + ':' + name)),
          renderOnly: typeof render.parseJson === 'undefined',
          offset: utf16ToUtf8('😀é', 2),
        }
      }, declared)
      assert.deepEqual(result.missingExports, [])
      assert.deepEqual(result.deep, ['document', 'document'])
      assert.deepEqual(result.rejected, [true, true])
      assert.equal(result.html[0], result.html[1])
      assert.match(result.html[0], /Browser/)
      for (const html of result.safe) assert.ok(!html.includes('<script>'))
      assert.equal(result.renderOnly, true)
      assert.equal(result.offset, 4)
      const worker = await page.evaluate(async () => {
        const call = (id, action, extra = {}) => new Promise(resolve => {
          const reply = event => { if (event.data.id === id) { window.worker.removeEventListener('message', reply); resolve(event.data) } }
          window.worker.addEventListener('message', reply)
          window.worker.postMessage({ id, action, ...extra })
        })
        const opened = await call(1, 'open', { source: 'First paragraph.\n\nSecond paragraph.\n' })
        const edited = await call(2, 'edit', { changes: [{ range: [0, 5], replacement: 'Other' }] })
        const failed = await call(3, 'edit', { changes: [{ range: [0, 99999], replacement: 'X' }] })
        const failedOpen = await call(4, 'open')
        const retained = await call(5, 'edit', { changes: [] })
        const rendered = await call(6, 'render', { source: '# Worker' })
        const closed = await call(7, 'close')
        const afterClose = await call(8, 'edit', { changes: [] })
        const reopened = await call(9, 'open', { source: 'Reopened.' })
        await call(10, 'close')
        window.worker.terminate()
        return { opened: opened.value.document.type, edited: edited.value.source, reused: edited.value.reusedPreviousTree, failed: failed.error.name, failedOpen: failedOpen.error.name, retained: retained.value.source, rendered: rendered.value, closed: closed.value, afterClose: afterClose.error.message, reopened: reopened.value.source }
      })
      assert.equal(worker.opened, 'document')
      assert.match(worker.edited, /^Other/)
      assert.equal(worker.reused, true)
      assert.equal(worker.failed, 'TypeError')
      assert.equal(worker.failedOpen, 'TypeError')
      assert.equal(worker.retained, worker.edited)
      assert.match(worker.rendered, /Worker/)
      assert.equal(worker.closed, null)
      assert.match(worker.afterClose, /Open a document/)
      assert.equal(worker.reopened, 'Reopened.')
      assert.deepEqual(errors, [])
      console.log(`consumer: ${name} loads the installed package, web and render entries`)
    } finally { await browser.close() }
  }
} finally { await new Promise(resolve => server.close(resolve)) }
console.log('consumer: installed tarball passes TypeScript, Node ESM/CJS and Vite browser checks')
