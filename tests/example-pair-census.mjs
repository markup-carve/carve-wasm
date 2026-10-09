// The population gate's counter, measured against a page whose answer is known.
//
// tests/corpus-source.mjs compares the corpus on disk against what the spec's
// example pages declare, and until markup-carve/carve#2824 it counted one pair
// per `::: compare` block. The generator writes one pair per `carve` fence
// inside a block, so a multi-pair block made the gate red on a corpus that was
// perfectly whole. The real corpus can no longer show it: the one multi-pair
// block on spec main was split by markup-carve/carve#2825, which is why the
// proof is this synthetic page rather than a corpus run.

import assert from 'node:assert/strict'

import { censusComparePairs, countDeclaredPairs } from './lib/example-pair-census.mjs'

const page = [
  '::: compare',
  '```carve',
  'one',
  '```',
  '```html',
  '<p>one</p>',
  '```',
  // A wider fence whose CONTENT is a carve fence. Nothing inside a fence is
  // markup, so this declares no pair of its own.
  '````carve',
  '```carve',
  'nested, not a pair',
  '```',
  '````',
  '```html',
  '<pre>two</pre>',
  '```',
  '```carve',
  'three',
  '```',
  '```html',
  '<p>three</p>',
  '```',
  ':::',
  // Outside every block, so it declares nothing either.
  '```carve',
  'outside any block',
  '```',
].join('\n')

const lines = page.split('\n')
const pairs = countDeclaredPairs(lines)
assert.equal(pairs, 3, `got ${pairs} pairs, want 3`)

const { blocks, openFence } = censusComparePairs(lines)
assert.equal(blocks.length, 1, `got ${blocks.length} compare blocks, want 1`)
assert.deepEqual(
  { carve: blocks[0].carve, html: blocks[0].html },
  { carve: 3, html: 3 },
  'the block declares three carve fences and three html fences',
)
assert.equal(openFence, null, 'every fence on the page closes')

// A block that never closes is reported rather than silently dropped: its pairs
// are still declared, and corpus-source.mjs refuses the page.
const unclosed = censusComparePairs(['::: compare', '```carve', 'one', '```'])
assert.equal(unclosed.blocks.length, 1)
assert.equal(unclosed.blocks[0].unclosed, true)
assert.equal(unclosed.blocks[0].carve, 1)

// A longer colon run opens a block too, and modifiers do not hide it.
const modified = countDeclaredPairs([
  ':::: compare no-render',
  '```carve',
  'one',
  '```',
  '::::',
])
assert.equal(modified, 1, `got ${modified} pairs for a modified block, want 1`)

console.log('example pair census: 3 declared pairs from one compare block, nested and outside fences ignored')
