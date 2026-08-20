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
// So this runs the deepest shape in a CHILD process with a deliberately small
// stack. Passing at a quarter of Node's default means a change that triples the
// frame size fails here, in a run that names the cause, instead of failing for
// a user on a machine with less headroom.
import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

// The engine's own nesting cap. A document past it is one paragraph, so this is
// the deepest tree that exists rather than an arbitrary large number.
const CAP = 200

// Kilobytes. Node's default is about 984, so this asserts roughly a quarter of
// it is enough. Measured on this artifact: it survives at 250 and overflows at
// 200, so 300 is above the floor without being so close that a rebuild flips it.
const SMALL_STACK = 300

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

// First at the default stack, so a failure here is unambiguous rather than a
// consequence of the flag below.
execFileSync(process.execPath, [self, '--child'], { stdio: 'inherit' })

try {
  execFileSync(process.execPath, [`--stack-size=${SMALL_STACK}`, self, '--child'], {
    stdio: 'inherit',
  })
} catch (error) {
  throw new Error(
    `the AST path needs more than ${SMALL_STACK}KB of host stack for a document at ` +
      `the ${CAP}-level nesting cap, where it used to fit. Node's default is about ` +
      `984KB, so the margin a browser or another Node build has just shrank. ` +
      `See markup-carve/carve-rs#1160.`,
    { cause: error },
  )
}

console.log(
  `deep nesting: ${Object.keys(shapes).length} shapes at the ${CAP}-level cap ` +
    `parse and serialize in ${SMALL_STACK}KB of host stack`,
)
