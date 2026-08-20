// The release gate: measure the package `npm publish` is about to upload.
//
// WHAT THIS REPLACES. release.yml ran `wasm-pack build` and then `npm publish`
// with nothing in between. No test, no corpus, no exit code that could stop it:
// whatever the build produced went to the registry, and a registry publish is
// not retractable in any meaningful sense - it is indexed within minutes and a
// deprecate or unpublish is itself a public event. That is the "check that
// cannot fail" shape catalogued in markup-carve/carve#755, sitting on the one
// step where failing late is most expensive.
//
// It is not hypothetical. carve-rb's last run on `main` was green while the
// artifact built from that same `main` rendered 24 of 1241 corpus documents
// wrongly, and the sibling framework packages (laravel-carve, symfony-carve,
// shopware-carve) carry a correct 1241-document check that runs on `schedule`
// only and reports divergence as a WARNING while exiting 0 - so every required
// check is green while the shipped engine renders 15.5 percent of the corpus
// differently.
//
// WHAT IT MEASURES, and why it packs first. `wasm-pack` writes `pkg/`, but
// `pkg/` is not what npm uploads: the generated package.json carries a `files`
// list, and a file left out of it is a file no test in this repo would ever
// have loaded. So the gate does not read `pkg/` directly. It runs `npm pack`,
// unpacks the tarball, and drives the SUITE THAT ALREADY EXISTS at the
// unpacked tree through CARVE_WASM_PKG. A file that cannot reach the registry
// therefore cannot reach the tests either, and the two agree by construction.
//
// The suite is reused rather than reimplemented, deliberately. tests/corpus.mjs
// already derives the corpus POPULATION from the spec's example pages instead
// of a written-down number, and already refuses a corpus of the wrong size
// rather than reporting a clean subset as a pass. A second spelling of that
// derivation in here would be a second thing to keep correct, and two
// population checks that disagreed would be a defect of its own.
//
// The exit code is the whole point. This script exits non-zero on any failure,
// and the workflow puts it in its own job that `publish` declares with
// `needs:` - not merely an earlier step in the same job, which a later
// `continue-on-error` or a reordering could quietly detach.
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const root = resolve(fileURLToPath(new URL('..', import.meta.url)))
const pkgDir = join(root, 'pkg')
const workDir = join(root, 'release-artifact')
const unpacked = join(workDir, 'package')

const die = (message) => {
  console.error(`release gate: ${message}`)
  process.exit(1)
}

const run = (command, args, options = {}) =>
  execFileSync(command, args, { encoding: 'utf8', stdio: 'pipe', ...options })

// Fail rather than skip. tests/corpus.mjs tolerates an unset CARVE_SPEC_CORPUS
// so a checkout without the spec repo can still run the suite; on a release
// that tolerance is the hole, because the run would publish having compared
// nothing. Refuse it here, before anything is built or packed.
if (!process.env.CARVE_SPEC_CORPUS) {
  die(
    'CARVE_SPEC_CORPUS is unset. The corpus is the only check that can tell a drifted engine ' +
      'from a current one, and a release must not be able to skip it. Check out ' +
      'markup-carve/carve and point CARVE_SPEC_CORPUS at its tests/corpus.',
  )
}
if (!existsSync(pkgDir) || !statSync(pkgDir).isDirectory()) {
  die(`no build at ${pkgDir}. Run the release build before this gate.`)
}

// ---------------------------------------------------------------------------
// Pack exactly what npm would upload.
rmSync(workDir, { recursive: true, force: true })
mkdirSync(workDir, { recursive: true })

let packed
try {
  packed = JSON.parse(run('npm', ['pack', '--json', '--pack-destination', workDir], { cwd: pkgDir }))
} catch (error) {
  die(`npm pack failed in ${pkgDir}: ${error.stderr || error.message}`)
}

const [meta] = packed
const tarball = join(workDir, meta.filename)
const digest = createHash('sha256').update(readFileSync(tarball)).digest('hex')

console.log(`release gate: packed ${meta.name}@${meta.version} as ${meta.filename}`)
console.log(`release gate: sha256 ${digest}`)
console.log(`release gate: ${meta.entryCount} file(s), ${meta.unpackedSize} bytes unpacked`)
for (const file of meta.files) console.log(`  ${file.path}`)

run('tar', ['-xzf', tarball, '-C', workDir])
if (!existsSync(unpacked)) die(`the tarball did not unpack to ${unpacked}`)

// The tarball's own listing is the population being tested, so state it rather
// than trusting the build: a `files` list that dropped the payload produces a
// tarball that is still a valid package, and the failure would otherwise
// surface as an obscure import error.
const shipped = new Set(readdirSync(unpacked))
const required = ['package.json', 'carve_wasm.js', 'carve_wasm_bg.wasm']
const absent = required.filter((name) => !shipped.has(name))
if (absent.length > 0) {
  die(
    `the tarball is missing ${absent.join(', ')}. npm uploads what the "files" list in ` +
      `${join(pkgDir, 'package.json')} names, so a payload file left out of it never reaches ` +
      'the registry and never reached these tests either.',
  )
}

// ---------------------------------------------------------------------------
// Drive the existing suite at the unpacked tarball.
const env = { ...process.env, CARVE_WASM_PKG: unpacked }
const suite = [
  ['tests/smoke.mjs', 'the hand-written API cases'],
  ['tests/corpus.mjs', 'the spec corpus, every document, byte-identical'],
  ['tests/deep-nesting.mjs', 'how much host stack the AST path needs'],
]

// `parseJson` recurses once per nesting level and the engine accepts 200
// levels, so the deepest corpus document sits close to what a host gives a Node
// process. The corpus run gets explicit headroom; how little is left over is
// what tests/deep-nesting.mjs measures, and markup-carve/carve-rs#1160 is what
// removes the recursion.
const STACK = ['--stack-size=4000']

for (const [file, what] of suite) {
  console.log(`\nrelease gate: ${file} - ${what}`)
  try {
    process.stdout.write(run(process.execPath, [file], { cwd: root, env, stdio: ['ignore', 'pipe', 'inherit'] }))
  } catch (error) {
    if (error.stdout) process.stdout.write(error.stdout)
    die(`${file} failed against the packed artifact. This release must not publish.`)
  }
}

console.log('\nrelease gate: the packed artifact passes; publishing is allowed')
