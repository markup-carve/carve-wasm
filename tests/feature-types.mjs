import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { readFileSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('..', import.meta.url))
const require = createRequire(import.meta.url)
for (const features of ['ast-json', 'ast-json,incremental', 'reports']) {
  const output = join(root, 'target', 'feature-types', features.replaceAll(',', '-'))
  execFileSync(process.env.WASM_PACK || 'wasm-pack', ['build', '--target', 'nodejs', '--release', '--no-opt', '--out-dir', output, '.', '--locked', '--no-default-features', '--features', features], { cwd: root, stdio: 'inherit', env: { ...process.env, CARGO_TARGET_DIR: join(root, 'target', 'feature-types', 'cargo') } })
  const manifestPath = join(output, 'package.json')
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
  manifest.type = 'commonjs'
  writeFileSync(manifestPath, JSON.stringify(manifest))
  const api = require(join(output, 'carve_wasm.js'))
  const declaration = readFileSync(join(output, 'carve_wasm.d.ts'), 'utf8')
  for (const [, name] of declaration.matchAll(/export (?:function|class) (\w+)/g)) {
    assert.equal(typeof api[name], 'function', `${features} declares absent ${name}`)
  }
  assert.equal(typeof api.astJsonToMarkdown, 'undefined')
  assert.equal(typeof api.toHtmlWithReport, features === 'reports' ? 'function' : 'undefined')
  assert.equal(typeof api.ParserSession, features.includes('incremental') ? 'function' : 'undefined')
  console.log(`feature types: ${features} declarations match the runtime`)
}
