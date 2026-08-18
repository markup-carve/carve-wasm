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
import { toHtml, parseJson } from '../pkg/carve_wasm.js'

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

// ---------------------------------------------------------------------------
// The same corpus, through `parseJson` rather than `toHtml`.
//
// The comparison above cannot see an AST-only change: a node that renders
// nothing renders nothing in both engines. carve-rb sat 44 commits behind on a
// pin that had lost a whole node type and every corpus pair still matched
// (markup-carve/carve-rb#46, markup-carve/carve-wasm#24).
//
// Two checks, answering different questions. A pin that drops a TYPE drops it
// from all 658 documents at once, so the whole corpus answers with one fact and
// no per-document reasoning. A pin that RENAMES a field keeps the type and is
// invisible to that, so the field names are checked against the spec's schema.
//
// The schema check is not JSON Schema validation: it reads two keywords,
// `additionalProperties: false` and `required`, and ignores types, enums,
// formats and conditionals. Those two are what a drifted engine actually trips.
//
// WHY NOT A PER-DOCUMENT ASSERTION. "A document with a definition line must
// produce a link_reference_definition" does not survive the corpus: 64
// documents have that source shape and 36 legitimately produce no such node -
// `[^f]: note` is the same shape, and several documents exist precisely to pin
// that a definition-shaped line is NOT a definition.

const EXPECTED_TYPES = `
  abbreviation abbreviation_def admonition autolink block_quote caption_number
  code code_block comment critic_comment definition_description definition_list
  definition_term delete div document emphasis escaped_text figure figure_group
  footnote footnote_ref frontmatter hard_break heading heading_ref highlight image
  inline_extension inline_footnote insert line_block link
  link_reference_definition list list_item literal_inline math mention paragraph
  raw_block raw_inline smart_punctuation soft_break span strike strong subscript
  substitution superscript symbol table table_cell table_row tag text
  thematic_break underline
`.trim().split(/\s+/)

const walk = (node, visit) => {
  if (Array.isArray(node)) {
    for (const value of node) walk(value, visit)
  } else if (node && typeof node === 'object') {
    if (typeof node.type === 'string') visit(node)
    for (const value of Object.values(node)) walk(value, visit)
  }
}

const trees = names.map((name) => JSON.parse(parseJson(readFileSync(join(CORPUS, name), 'utf8'))))

const produced = new Set()
for (const tree of trees) walk(tree, (node) => produced.add(node.type))

const missingTypes = EXPECTED_TYPES.filter((type) => !produced.has(type))
assert.deepEqual(
  missingTypes,
  [],
  `${missingTypes.length} node type(s) the corpus used to produce are gone: ${missingTypes.join(', ')}. ` +
    'The carve-rs rev in Cargo.toml is probably behind a change that renamed or removed them; ' +
    'bump it and commit the regenerated Cargo.lock. If a type was removed from the language on ' +
    'purpose, delete it from EXPECTED_TYPES in the same commit.',
)

// The ablation for the check above: without it, that assertion passes
// identically whether the trees were walked or the walk quietly found nothing.
assert.ok(!produced.has('a_type_no_engine_emits'), 'the type sweep is not reading the trees')
assert.ok(produced.size >= EXPECTED_TYPES.length, 'the type sweep found fewer types than it records')

const schemaPath = join(CORPUS, '..', '..', 'resources', 'ast-schema.json')
const defs = JSON.parse(readFileSync(schemaPath, 'utf8'))['$defs']
assert.ok(
  Object.keys(defs).length >= 40,
  `the schema at ${schemaPath} has only ${Object.keys(defs).length} definitions, which is too few to be the spec's`,
)

const schemaFindings = (definitions) => {
  const found = new Map()
  const note = (label) => found.set(label, (found.get(label) ?? 0) + 1)
  for (const tree of trees) {
    walk(tree, (node) => {
      const schema = definitions[node.type]
      if (!schema) return note(`${node.type}: no $defs entry in the schema`)
      if (!schema.properties) return
      if (schema.additionalProperties === false) {
        for (const key of Object.keys(node)) {
          if (!(key in schema.properties)) note(`${node.type}.${key}: not a property the schema names`)
        }
      }
      for (const required of schema.required ?? []) {
        if (!(required in node)) note(`${node.type}: required property ${required} is missing`)
      }
    })
  }
  return found
}

const fieldFindings = schemaFindings(defs)
assert.equal(
  fieldFindings.size,
  0,
  `nodes do not match the schema's field names: ${[...fieldFindings].slice(0, 10).map(([label, n]) => `${n}x ${label}`).join('; ')}. ` +
    'The carve-rs rev in Cargo.toml is probably behind a rename.',
)

// The ablation for the field check, for the same reason: a schema that moved
// would otherwise make it pass having compared nothing.
const mutated = { ...defs, text: { ...defs.text, properties: { ...defs.text.properties } } }
delete mutated.text.properties.value
assert.ok(
  schemaFindings(mutated).has('text.value: not a property the schema names'),
  'the schema sweep is not reading the schema',
)

console.log(`corpus: ${names.length} documents parsed, ${produced.size} node types, field names match the schema`)
