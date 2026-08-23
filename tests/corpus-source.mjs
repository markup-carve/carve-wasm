// The corpus population, derived once and read by every gate that measures it.
//
// This block used to live inside tests/corpus.mjs. tests/roundtrip.mjs needs the
// same three things - the corpus path, the refusal to skip in CI, and how many
// documents there SHOULD be - and that file already says why a second spelling
// would be a defect rather than a duplication: "two spellings of 'how big is the
// corpus' that could disagree would be its own defect". A second copy would also
// be a second place for the CI skip-refusal to be forgotten, and forgetting it is
// the whole failure it guards.
//
// Nothing about the rules below changed in the move. The comments are the
// originals, because they carry the incidents the rules came from.

import assert from 'node:assert/strict'
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'

const CORPUS = process.env.CARVE_SPEC_CORPUS

// The skip above is a convenience for a local checkout without the spec repo.
// In CI it is a hole: this whole file is reached through one `env:` block in
// ci.yml, and if that block is deleted, renamed, or moved to a job that does
// not check the spec out, every assertion below stops running and the step
// still exits 0. The job goes green having compared nothing, which is the
// variant-1 dead check from markup-carve/carve#755 - the same shape hugo-carve
// shipped, where ci.yml skipped the corpus test in silence.
//
// So the skip is allowed exactly where it is useful and refused where it is
// dangerous. A CI runner that reaches this file without a corpus is a wiring
// failure, and it fails here rather than being reported as a pass.
const IN_CI = process.env.GITHUB_ACTIONS === 'true' || process.env.CI === 'true'

if (!CORPUS) {
  assert.ok(
    !IN_CI,
    'CARVE_SPEC_CORPUS is unset in a CI run. This file is the only check that measures the ' +
      'built wasm artifact against the spec, and without the corpus it asserts nothing. Set it ' +
      'from the spec checkout (see the "Check out the spec corpus" step in ' +
      '.github/workflows/ci.yml); do not let this run report success.',
  )
  console.log('corpus: skipped (CARVE_SPEC_CORPUS not set - see .github/workflows/ci.yml)')
  process.exit(0)
}

// How many documents the corpus SHOULD hold, derived rather than written down.
//
// An empty or short corpus otherwise reports zero mismatches and reads as a
// pass, which is the failure mode this whole job exists to replace. The guard
// used to be `>= 400` against a corpus of over a thousand, which is the same
// dead check wearing a number: measured on this repo, a corpus cut to 420
// documents printed `corpus: 420/420 documents byte-identical` with all 45
// then-diverging documents simply absent (markup-carve/carve-wasm#36, the
// variant-2 shape catalogued in markup-carve/carve#755).
//
// THE REFERENCE HAS TO BE SOMETHING THIS RUNNER DOES NOT ITSELF READ AS THE
// POPULATION. Deriving "how many should there be" from the corpus directory
// would move both sides of the comparison together and guard nothing. So the
// reference is the corpus's SOURCE: tests/corpus is generated from the
// `::: compare` blocks in resources/examples/{core,extensions,edge-cases}.md
// (see scripts/generate-corpus.mjs in the spec repository), and the generator
// refuses to write a corpus where the two disagree. They sit in the same spec
// checkout, reached the same way this file already reaches
// resources/ast-schema.json.
//
// Counting the source also leaves no literal here to go stale: adding an
// example upstream moves the expectation on the next spec checkout, without
// anyone editing this file. A hardcoded 1053 would be this same defect with a
// bigger number.
const EXAMPLE_PAGES = ['core.md', 'extensions.md', 'edge-cases.md']

// Mirrors the generator: `::: compare`, or a longer colon run, with optional
// modifiers such as `::: compare no-render`.
const COMPARE_OPEN = /^:{3,}\s+compare(\s+\S.*)?$/

// The scan mirrors the generator's state machine rather than grepping: a
// `::: compare` line inside an already-open block is content, not a second
// pair, and a block closes on a bare marker line. Mirroring keeps the two
// counts equal by construction instead of by luck.
const declaredCorpusSize = (corpusDir) => {
  const examplesDir = join(corpusDir, '..', '..', 'resources', 'examples')
  let declared = 0
  for (const page of EXAMPLE_PAGES) {
    const path = join(examplesDir, page)
    let blob
    try {
      blob = readFileSync(path, 'utf8')
    } catch (error) {
      // Not a soft skip. Without this page there is no independent statement of
      // how big the corpus should be, and a corpus check with nothing to
      // compare against is the failure shape this guard exists to remove.
      assert.fail(
        `no corpus source page at ${path}: ${error.message}. tests/corpus is generated from ` +
          'these pages; if the spec moved them, this helper has to move with them.',
      )
    }
    let marker = null
    for (const rawLine of blob.split('\n')) {
      const line = rawLine.trim()
      if (marker !== null) {
        if (line === marker) marker = null
        continue
      }
      if (COMPARE_OPEN.test(line)) {
        declared += 1
        marker = line.match(/^:{3,}/)[0]
      }
    }
  }
  assert.ok(
    declared > 0,
    `the corpus source pages under ${examplesDir} declare no ::: compare blocks at all; ` +
      'that is a wiring problem, not a corpus of size zero.',
  )
  return declared
}

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

// EQUALITY, not a floor. A floor cannot tell a whole corpus from a truncated or
// stale one, and truncation is the failure being guarded against.
const declared = declaredCorpusSize(CORPUS)
assert.equal(
  names.length,
  declared,
  `${names.length} corpus pairs under ${CORPUS}, but the spec's example pages declare ${declared}. ` +
    'Every ::: compare block in resources/examples/{core,extensions,edge-cases}.md becomes one ' +
    'corpus pair, so a difference means the corpus checked out here is not the one those pages ' +
    'describe - a truncated or stale checkout, a wrong CARVE_SPEC_CORPUS, or a corpus that needs ' +
    'regenerating (npm run corpus:build in the spec repository). It does not mean this run was clean.',
)

export { CORPUS, names, declared }
