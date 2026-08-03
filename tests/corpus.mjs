// The mandatory spec corpus, rendered through the BUILT wasm artifact.
//
// `cargo test` exercises the native build of the wrapper and `smoke.mjs`
// exercises the artifact against hand-written expectations. Neither can tell a
// drifted engine pin from a current one: a stale carve-rs satisfies a
// hand-written expectation happily, which is how carve-py's pin sat months
// behind with CI green, and how carve-go shipped an embedded engine nobody
// could date. Only the corpus measures the artifact against the spec.
//
// The engine underneath is itself corpus-checked upstream. That is not the
// claim here - the claim is that the wasm-bindgen BINDING does not lose
// anything on the way through, and nothing else tests that.
//
// The corpus path comes from CARVE_SPEC_CORPUS. Unset, this exits 0 with a
// notice, so a plain checkout without the spec repo still runs the suite; CI
// always sets it.
import assert from 'node:assert/strict'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { toHtml } from '../pkg/carve_wasm.js'

const CORPUS = process.env.CARVE_SPEC_CORPUS

if (!CORPUS) {
  console.log('corpus: skipped (CARVE_SPEC_CORPUS not set - see .github/workflows/ci.yml)')
  process.exit(0)
}

// The corpus has ~500 pairs. Far below that means the path is wrong rather than
// that the run was clean - an empty directory otherwise reports zero mismatches
// and reads as a pass, which is the failure mode this whole job exists to
// replace.
const MIN_PAIRS = 400

let entries
try {
  entries = readdirSync(CORPUS)
} catch (error) {
  assert.fail(`CARVE_SPEC_CORPUS=${CORPUS} is not readable: ${error.message}`)
}
assert.ok(statSync(CORPUS).isDirectory(), `CARVE_SPEC_CORPUS=${CORPUS} is not a directory`)

// Pair by BASENAME, never by slicing the joined path: `join()` normalizes a
// trailing slash on CARVE_SPEC_CORPUS away, so a path-length slice would drop a
// character and match nothing - reporting a wiring failure for a path that is
// perfectly valid, which is exactly the kind of unfalsifiable red this check
// exists to avoid.
const present = new Set(entries)
const names = entries
  .filter((name) => name.endsWith('.crv'))
  .filter((name) => present.has(`${name.slice(0, -4)}.html`))
  .sort()

assert.ok(
  names.length >= MIN_PAIRS,
  `only ${names.length} corpus pairs under ${CORPUS}; the corpus has ~500, so this is a wiring problem, not a clean run`,
)

const trimmed = (path) => readFileSync(path, 'utf8').replace(/\n+$/, '')
const render = (name) => toHtml(readFileSync(join(CORPUS, name), 'utf8')).replace(/\n+$/, '')

const mismatches = names.filter((name) => render(name) !== trimmed(join(CORPUS, `${name.slice(0, -4)}.html`)))

if (mismatches.length > 0) {
  const [first] = mismatches
  console.error(`--- ${first} ---`)
  console.error(`got:  ${JSON.stringify(render(first))}`)
  console.error(`want: ${JSON.stringify(trimmed(join(CORPUS, `${first.slice(0, -4)}.html`)))}`)
}
assert.equal(
  mismatches.length,
  0,
  `${mismatches.length}/${names.length} corpus documents differ: ${mismatches.slice(0, 10).join(', ')}`,
)
console.log(`corpus: ${names.length}/${names.length} documents byte-identical through the wasm artifact`)
