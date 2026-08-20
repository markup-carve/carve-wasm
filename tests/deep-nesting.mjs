// How much host stack the AST path needs, as a number the suite reports.
//
// `parseJson` walks the tree recursively, one host frame per nesting level, and
// the engine accepts documents nested to its own cap - so the deepest document
// it will parse is also the deepest recursion it will perform. On wasm the
// limit is the HOST's stack, which a browser or a Node build decides and this
// package cannot size: `-z stack-size` in .cargo/config.toml sizes the shadow
// stack in linear memory, and this overflow is V8's native stack raising a
// RangeError.
//
// That is markup-carve/carve-rs#1160, and until it is fixed the honest thing
// here is to MEASURE the headroom rather than assume it. carve-wasm CI flipped
// from green to red on the same commit, same rustc, same wasm-pack and same
// Node (markup-carve/carve-wasm#48) - which is what a thin margin looks like
// from outside.
//
// So this MEASURES the floor - the smallest host stack the deepest shapes still
// fit in - by running them in a child process at decreasing sizes, and prints
// it. A fixed threshold would only have described the machine it was written
// on: 250KB is enough here and not enough on a GitHub runner, for the same
// commit and the same artifact.
//
// What it FAILS on is the floor climbing past a ceiling - a stable signal, and
// one this repository can act on. What it WARNS about is the default stack
// overflowing, which is the user-visible defect and is not this repository's to
// fix: it is markup-carve/carve-rs#1160, and it flips run to run on a GitHub
// runner today. Failing on it would make this a gate that cannot pass, and a
// gate nobody can pass is a gate nobody reads.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

// The engine's own nesting cap. A document past it is one paragraph, so this is
// the deepest tree that exists rather than an arbitrary large number.
const CAP = 200

// Kilobytes, largest first. Node's default is about 984.
const PROBES = [900, 700, 500, 400, 300, 250, 200, 150]

// The floor may not exceed this, or a default-stack run has almost nothing
// left over - which is the state CI was in when the corpus flipped from green
// to red on an unchanged commit.
const CEILING = 900

const nested = ':::: note\n'.repeat(CAP) + 'deep\n' + '::::\n'.repeat(CAP)

const shapes = {
  'nested containers': nested,
  'nested quotes': '> '.repeat(CAP) + 'deep\n',
  'nested lists': Array.from({ length: CAP }, (_, i) => ' '.repeat(i * 2) + '- x').join('\n') + '\n',
}

// The floor each shape must still reach, well under what it produces today
// (containers 406, quotes 406, lists 805). Without this the test measures
// nothing the day a parser change flattens one of the shapes: a shallow
// document serializes in any stack, and a green run would mean "the recursion
// is fine" when it means "there was no recursion".
const DEPTH_FLOOR = 300

/** JSON nesting depth, walked with an explicit stack - a recursive walk here
 *  would hit the very limit this file is about. */
function depth(root) {
  let deepest = 0
  const pending = [[root, 0]]
  while (pending.length) {
    const [node, level] = pending.pop()
    if (level > deepest) deepest = level
    if (Array.isArray(node)) {
      for (const child of node) pending.push([child, level + 1])
    } else if (node && typeof node === 'object') {
      for (const key of Object.keys(node)) pending.push([node[key], level + 1])
    }
  }
  return deepest
}

if (process.argv[2] === '--child') {
  // Runs under the reduced stack; the parent reads the exit status.
  const { parseJson, toHtml } = await import('./engine.mjs')
  for (const [name, source] of Object.entries(shapes)) {
    const html = toHtml(source)
    assert.ok(html.length > 0, `${name}: toHtml produced nothing`)
    const tree = JSON.parse(parseJson(source))
    assert.equal(tree.type, 'document', `${name}: parseJson did not return a document`)
    const levels = depth(tree)
    assert.ok(
      levels >= DEPTH_FLOOR,
      `${name}: the tree is ${levels} levels deep, under the ${DEPTH_FLOOR} this ` +
        `test needs to be measuring anything. The input stopped nesting - a syntax ` +
        `change, a lower cap, or a parser regression - so a pass here would say the ` +
        `stack is fine when nothing deep was serialized.`,
    )
  }
  process.exit(0)
}

const self = fileURLToPath(import.meta.url)

function fitsIn(kilobytes) {
  const flags = kilobytes === null ? [] : [`--stack-size=${kilobytes}`]
  try {
    execFileSync(process.execPath, [...flags, self, '--child'], { stdio: 'pipe' })
    return true
  } catch (error) {
    const output = `${error.stderr ?? ''}`
    if (output.includes('Maximum call stack size exceeded')) return false
    // Anything else is a real failure and must not read as "needs more stack".
    process.stderr.write(output)
    throw error
  }
}

// The contract, reported rather than enforced: a document the engine accepts
// should serialize at the stack a consumer actually has. On a GitHub runner
// this flips between runs today, which is the whole reason the floor below is
// measured instead of assumed.
const fitsAtDefault = fitsIn(null)
if (!fitsAtDefault) {
  console.warn(
    `WARNING: the AST path overflowed the DEFAULT host stack on a document at ` +
      `the ${CAP}-level nesting cap. A consumer calling parseJson on a deep ` +
      `document crashes on this machine. That is markup-carve/carve-rs#1160 - ` +
      `the walk recurses once per nesting level - and it is not fixable here.`,
  )
}

let floor = null
for (const probe of PROBES) {
  if (!fitsIn(probe)) break
  floor = probe
}

if (floor === null) {
  throw new Error(
    `the AST path needs more than ${PROBES[0]}KB of host stack for a document at ` +
      `the ${CAP}-level nesting cap. Node's default is about 984KB, so a default ` +
      `run has almost nothing left over - which is the state this repository's CI ` +
      `was in when the corpus flipped from green to red on an unchanged commit. ` +
      `See markup-carve/carve-rs#1160.`,
  )
}

assert.ok(
  floor <= CEILING,
  `the AST path fits in ${floor}KB, above the ${CEILING}KB this test allows.`,
)

console.log(
  `deep nesting: ${Object.keys(shapes).length} shapes at the ${CAP}-level cap ` +
    `parse and serialize in ${floor}KB of host stack (Node's default is ~984KB, ` +
    `and it ${fitsAtDefault ? 'fits there' : 'DID NOT fit there on this run'})`,
)
