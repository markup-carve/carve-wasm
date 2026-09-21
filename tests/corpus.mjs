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
//
// WHICH built artifact is a parameter too - see tests/engine.mjs. Unset it is
// `../pkg`, exactly as before; the release gate points CARVE_WASM_PKG at the
// unpacked npm tarball so this same file, with this same population
// derivation, measures the bytes about to be published. One derivation, two
// callers: two spellings of "how big is the corpus" that could disagree would
// be its own defect.
import assert from 'node:assert/strict'
import { createHash } from 'node:crypto'
import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { astJsonToHtml, toHtml, parseJson, packageUnderTest } from './engine.mjs'

import { CORPUS, names } from './corpus-source.mjs'

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
console.log(`corpus: ${names.length}/${names.length} documents byte-identical through the wasm artifact at ${packageUnderTest}`)

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
// WHY NOT A PER-DOCUMENT ASSERTION FROM THE SOURCE SHAPE. "A document with a
// definition line must produce a link_reference_definition" does not survive
// the corpus: 64 documents have that source shape and 36 legitimately produce
// no such node - `[^f]: note` is the same shape, and several documents exist
// precisely to pin that a definition-shaped line is NOT a definition.
//
// Both checks are still AGGREGATES, and that was measured: a greedy cover of
// the 58 produced types needs 18 documents, so the other 1722 could return an
// empty tree and both would pass (markup-carve/carve-wasm#139). Three
// per-document checks close that, and only the third records anything.

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

// Named, one document at a time, rather than as a bare `map`. A parse that
// throws inside the wasm module - a stack overflow on a deeply nested document
// is the case on record - otherwise ends the run with a stack of
// `wasm://wasm/...` frames and no document name anywhere in it, so the only way
// to find the input was to bisect the corpus by hand on a runner. The name is
// the whole diagnosis, and it costs one try/catch.
const trees = names.map((name) => {
  const source = readFileSync(join(CORPUS, name), 'utf8')
  try {
    return JSON.parse(parseJson(source))
  } catch (error) {
    throw new Error(
      `parseJson failed on ${name} (${source.length} bytes): ${error.message}. ` +
        'The HTML half of this file passed over the same document, so this is the AST path ' +
        'specifically - a wasm-side failure such as a stack overflow on deep nesting, or ' +
        'output that is not JSON.',
      { cause: error },
    )
  }
})

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

// ---------------------------------------------------------------------------
// PER DOCUMENT, three checks. The two above are aggregates over the union of
// 1740 trees; these ask something of each tree on its own.

// ONE: the tree renders to the same HTML the direct path does.
//
// `parseJson` and `astJsonToHtml` are separate exports across the wasm
// boundary, and `toHtml` is a third; the equality ties the tree to the
// spec-owned golden the HTML half compared at the top of this file. So this is
// a per-document assertion about the AST whose expectation comes from the spec
// rather than from a recording of what the engine did.
const renderDiverges = names.filter((name, index) => {
  const source = readFileSync(join(CORPUS, name), 'utf8')
  return astJsonToHtml(JSON.stringify(trees[index])) !== toHtml(source)
})
assert.deepEqual(
  renderDiverges, [],
  `${renderDiverges.length} document(s) render differently from their own tree than from source: ` +
    `${renderDiverges.slice(0, 10).join(', ')}. The HTML half of this file compares the source path ` +
    'against the spec golden, so a tree that does not reproduce it has lost something on the AST path.',
)

// TWO: every position is what the SOURCE says it is.
//
// Nothing else here can see a position at all - the renderer ignores them and
// the two aggregates read type and field NAMES. They are 75288 numbers over
// 12548 nodes, and they are the one part of the tree with an external oracle:
// recompute each line and column from the source and compare.
//
// UNITS, because they are the contract and they have moved before: an offset is
// a 0-based Unicode CODEPOINT index, a column is a 1-based codepoint count from
// the line start, and a line is terminated by LF, CRLF or a lone CR.
const positionGrid = (text) => {
  const points = [...text]
  const line = new Int32Array(points.length + 1)
  const column = new Int32Array(points.length + 1)
  let atLine = 1
  let atColumn = 1
  for (let index = 0; index <= points.length; index += 1) {
    line[index] = atLine
    column[index] = atColumn
    if (index === points.length) break
    const point = points[index]
    if (point === '\n' || (point === '\r' && points[index + 1] !== '\n')) {
      atLine += 1
      atColumn = 1
    } else {
      atColumn += 1
    }
  }
  return { line, column, length: points.length }
}

const positionFindings = []
let positionsRead = 0
names.forEach((name, index) => {
  const { line, column, length } = positionGrid(readFileSync(join(CORPUS, name), 'utf8'))
  const report = (what) => positionFindings.push(`${name}: ${what}`)
  const check = (node, parent) => {
    const at = node.pos
    if (!at) return
    positionsRead += 1
    if (!(Number.isInteger(at.startOffset) && at.startOffset >= 0
      && at.startOffset <= at.endOffset && at.endOffset <= length)) {
      report(`${node.type} spans ${at.startOffset}..${at.endOffset} outside 0..${length}`)
      return
    }
    if (line[at.startOffset] !== at.startLine || column[at.startOffset] !== at.startColumn) {
      report(`${node.type} starts at offset ${at.startOffset}, which the source puts at `
        + `${line[at.startOffset]}:${column[at.startOffset]}, not ${at.startLine}:${at.startColumn}`)
    }
    if (line[at.endOffset] !== at.endLine || column[at.endOffset] !== at.endColumn) {
      report(`${node.type} ends at offset ${at.endOffset}, which the source puts at `
        + `${line[at.endOffset]}:${column[at.endOffset]}, not ${at.endLine}:${at.endColumn}`)
    }
    if (parent?.pos && (at.startOffset < parent.pos.startOffset || at.endOffset > parent.pos.endOffset)) {
      report(`${node.type} spans ${at.startOffset}..${at.endOffset}, outside its ${parent.type} parent `
        + `at ${parent.pos.startOffset}..${parent.pos.endOffset}`)
    }
  }
  const descend = (value, parent) => {
    if (Array.isArray(value)) {
      for (const item of value) descend(item, parent)
    } else if (value && typeof value === 'object') {
      let next = parent
      if (typeof value.type === 'string') {
        check(value, parent)
        next = value
      }
      for (const child of Object.values(value)) descend(child, next)
    }
  }
  descend(trees[index], null)
})
assert.deepEqual(
  positionFindings.slice(0, 10), [],
  `${positionFindings.length} node position(s) disagree with the source they index. Offsets are ` +
    'codepoints, columns are codepoints from the line start, and a child span sits inside its ' +
    "parent's. A change of unit is the case on record.",
)

// The ablation: a tree that stopped carrying positions would satisfy the check
// above having compared nothing, which is how a check quietly becomes
// decoration. One positioned node per document is far below the reading of the
// day and still impossible for a tree with no positions at all.
assert.ok(
  positionsRead >= names.length,
  `only ${positionsRead} node(s) across ${names.length} documents carry a position, so the check ` +
    'above compared almost nothing. Either the field was renamed or the engine stopped emitting it.',
)

// THREE: the tree itself, recorded per document.
//
// The two checks above cover what the renderer reads and what the source can
// arbitrate. What is left is the fields neither sees - `thematic_break.marker`,
// `list.olType`, `comment.delimited`, `strong.boldItalic` and the rest of the
// source-spelling facts an AST consumer reads and HTML cannot show. Nothing
// derives those, so they are RECORDED: this ledger says what the engine
// produces today and not what it ought to, and its worth is that a change to it
// cannot land without a reviewer approving the diff.
//
// The entry is a hash of the tree under sorted keys, plus the node types it
// holds. The signature is what makes a regeneration diff readable - a bump that
// moves a type shows as a type - and the hash is what catches the rest.
const AST_LEDGER = fileURLToPath(new URL('./ast-ledger.json', import.meta.url))
const REGENERATE_AST = 'UPDATE_AST_LEDGER=1 node --stack-size=4000 tests/corpus.mjs'

const canonical = (value) => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(',')}]`
  if (value && typeof value === 'object') {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`).join(',')}}`
  }
  return JSON.stringify(value)
}

const fingerprint = (tree) => {
  const counts = new Map()
  walk(tree, (node) => counts.set(node.type, (counts.get(node.type) ?? 0) + 1))
  const signature = [...counts]
    .sort(([left], [right]) => (left < right ? -1 : 1))
    .map(([type, count]) => (count === 1 ? type : `${type}:${count}`))
    .join(' ')
  const digest = createHash('sha256').update(canonical(tree)).digest('hex').slice(0, 16)
  return `${digest} ${signature}`
}

const fingerprints = Object.fromEntries(names.map((name, index) => [name, fingerprint(trees[index])]))

if (process.env.UPDATE_AST_LEDGER === '1') {
  const previous = existsSync(AST_LEDGER) ? JSON.parse(readFileSync(AST_LEDGER, 'utf8')) : {}
  writeFileSync(AST_LEDGER, `${JSON.stringify({
    $comment: previous.$comment ?? '',
    trees: fingerprints,
  }, null, 2)}\n`, 'utf8')
  console.log(`corpus: AST ledger rewritten - ${names.length} trees`)
  process.exit(0)
}

assert.ok(existsSync(AST_LEDGER), `no AST ledger at ${AST_LEDGER} - regenerate it: ${REGENERATE_AST}`)
const recordedTrees = JSON.parse(readFileSync(AST_LEDGER, 'utf8')).trees ?? {}

const unrecordedTrees = names.filter((name) => !(name in recordedTrees))
assert.deepEqual(
  unrecordedTrees, [],
  `${unrecordedTrees.length} document(s) have no recorded tree: ${unrecordedTrees.slice(0, 10).join(', ')}. ` +
    `The corpus grew; regenerate the ledger: ${REGENERATE_AST}`,
)
const phantomTrees = Object.keys(recordedTrees).filter((name) => !names.includes(name))
assert.deepEqual(
  phantomTrees, [],
  `${AST_LEDGER} records trees for documents the corpus does not have: ` +
    `${phantomTrees.slice(0, 10).join(', ')}`,
)
const movedTrees = names
  .filter((name) => recordedTrees[name] !== fingerprints[name])
  .map((name) => `${name}: recorded ${recordedTrees[name]}, now ${fingerprints[name]}`)
assert.deepEqual(
  movedTrees.slice(0, 5), [],
  `${movedTrees.length} document(s) parse to a different tree than the ledger records. The two ` +
    'aggregates above cannot see this: they pass as long as every type appears SOMEWHERE. If the ' +
    `new trees are right - an engine bump usually means they are - regenerate: ${REGENERATE_AST}`,
)

console.log(
  `corpus: ${names.length} documents parsed, ${produced.size} node types, field names match the ` +
    'schema, every tree renders as its source does, every position agrees with the source, and ' +
    'every tree matches its recorded fingerprint',
)
