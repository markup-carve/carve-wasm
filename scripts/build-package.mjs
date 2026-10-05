import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = fileURLToPath(new URL('..', import.meta.url))
const pkg = join(root, 'pkg')
const work = join(root, 'target', 'npm-build')
const noOpt = process.argv.includes('--no-opt')
rmSync(pkg, { recursive: true, force: true })
rmSync(work, { recursive: true, force: true })
mkdirSync(work, { recursive: true })
function build(name, target, renderOnly, optimize) {
  const output = join(work, name)
  const args = ['build', '--target', target, '--release', '--scope', 'markup-carve', '--out-dir', output]
  if (!optimize) args.push('--no-opt')
  args.push('.', '--locked')
  if (renderOnly) args.push('--no-default-features')
  execFileSync(process.env.WASM_PACK || 'wasm-pack', args, { cwd: root, stdio: 'inherit' })
  rmSync(join(output, '.gitignore'), { force: true })
  return output
}
const bundler = build('bundler', 'bundler', false, false)
const node = build('node', 'nodejs', false, false)
const web = build('web', 'web', false, false)
const raw = readFileSync(join(bundler, 'carve_wasm_bg.wasm'))
for (const output of [node, web]) {
  assert.ok(raw.equals(readFileSync(join(output, 'carve_wasm_bg.wasm'))), 'full targets must have identical WASM before sharing their payload')
}
const full = noOpt ? bundler : build('optimized', 'bundler', false, true)
assert.equal(readFileSync(join(full, 'carve_wasm_bg.js'), 'utf8'), readFileSync(join(bundler, 'carve_wasm_bg.js'), 'utf8'), 'optimization must preserve the full binding glue')
const render = build('render', 'web', true, !noOpt)
cpSync(full, pkg, { recursive: true })
for (const [name, output] of [['node', node], ['web', web], ['render', render]]) {
  cpSync(output, join(pkg, name), { recursive: true })
}
for (const [directory, before, after] of [
  ['node', '${__dirname}/carve_wasm_bg.wasm', '${__dirname}/../carve_wasm_bg.wasm'],
  ['web', "new URL('carve_wasm_bg.wasm', import.meta.url)", "new URL('../carve_wasm_bg.wasm', import.meta.url)"],
]) {
  const entry = join(pkg, directory, 'carve_wasm.js')
  const source = readFileSync(entry, 'utf8')
  assert.equal(source.split(before).length, 2, `expected one ${directory} WASM path in generated glue`)
  writeFileSync(entry, source.replace(before, after))
  rmSync(join(pkg, directory, 'carve_wasm_bg.wasm'))
}
const manifest = JSON.parse(readFileSync(join(pkg, 'package.json'), 'utf8'))
const entry = directory => ({ types: `./${directory}/carve_wasm.d.ts`, default: `./${directory}/carve_wasm.js` })
manifest.exports = {
  '.': { node: entry('node'), types: './carve_wasm.d.ts', default: './carve_wasm.js' },
  './node': entry('node'), './web': entry('web'), './render': entry('render'),
  './offsets': { types: './examples/offsets.d.ts', default: './examples/offsets.mjs' },
  './carve_wasm.js': './carve_wasm.js', './carve_wasm_bg.js': './carve_wasm_bg.js',
  './carve_wasm_bg.wasm': './carve_wasm_bg.wasm', './carve_wasm.d.ts': './carve_wasm.d.ts',
  './node/carve_wasm_bg.wasm': './carve_wasm_bg.wasm',
  './web/carve_wasm_bg.wasm': './carve_wasm_bg.wasm',
  './web/*': './web/*', './node/*': './node/*', './render/*': './render/*',
  './package.json': './package.json',
}
manifest.files = [...manifest.files, 'node', 'web', 'render', 'docs', 'examples']
mkdirSync(join(pkg, 'docs'), { recursive: true })
for (const name of ['reference.md', 'compatibility.md']) cpSync(join(root, 'docs', name), join(pkg, 'docs', name))
const nodeManifestPath = join(pkg, 'node', 'package.json')
const nodeManifest = JSON.parse(readFileSync(nodeManifestPath, 'utf8'))
nodeManifest.type = 'commonjs'
writeFileSync(nodeManifestPath, JSON.stringify(nodeManifest, null, 2) + '\n')
cpSync(join(root, 'examples'), join(pkg, 'examples'), { recursive: true })
writeFileSync(join(pkg, 'package.json'), JSON.stringify(manifest, null, 2) + '\n')
console.log(`package: bundler, Node and web share one full WASM payload; rendering-only entry at ${resolve(pkg)}`)
