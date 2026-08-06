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
  definition_term delete div document emphasis escaped_text figure footnote
  footnote_ref frontmatter hard_break heading heading_ref highlight image
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
