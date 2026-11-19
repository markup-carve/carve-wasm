import assert from 'node:assert/strict'
import { chromium } from 'playwright'
import { createServer } from 'vite'

const server = await createServer({ server: { host: '127.0.0.1', port: 0 }, logLevel: 'error' })
await server.listen()
const address = server.httpServer.address()
const browser = await chromium.launch({ headless: true })
try {
  const page = await browser.newPage()
  await page.goto(`http://127.0.0.1:${address.port}/tests/v2-browser.html`)
  const result = await page.evaluate(async () => {
    const wasm = await import('/pkg/web/carve_wasm.js')
    await wasm.default()
    const renderOnly = await import('/pkg/render/carve_wasm.js')
    await renderOnly.default()
    const tree = wasm.parse('# Browser')
    const worker = new Worker('/examples/carve-worker.mjs', { type: 'module' })
    const reply = await new Promise((resolve, reject) => {
      worker.onmessage = event => resolve(event.data)
      worker.onerror = reject
      worker.postMessage({ id: 1, action: 'render', source: 'a *b*' })
    })
    worker.terminate()
    return {
      html: wasm.render('# Browser'),
      treeType: tree.type,
      safe: wasm.render('```=html\n<script>alert(1)</script>\n```'),
      reply,
      renderOnly: renderOnly.toHtml('Hello'),
      renderOnlyParse: typeof renderOnly.parse,
    }
  })
  assert.match(result.html, /<h1>Browser<\/h1>/)
  assert.equal(result.treeType, 'document')
  assert.ok(!result.safe.includes('<script>'))
  assert.equal(result.reply.value, '<p>a <strong>b</strong></p>')
  assert.equal(result.renderOnly, '<p>Hello</p>')
  assert.equal(result.renderOnlyParse, 'undefined')
  console.log('browser WASM and worker smoke: pass')
} finally {
  await browser.close()
  await server.close()
}
