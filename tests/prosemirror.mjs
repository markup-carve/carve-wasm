// The ProseMirror bridge, driven over the whole corpus.
//
// markup-carve/carve-wasm#101 landed the bridge with 16 assertions on
// hand-written documents. Nothing exercised it at corpus scale, so a regression
// arriving with an engine bump had nothing here to fail (#102).
//
// WHAT IS COMPARED, and why it is canonical SOURCE rather than HTML.
//
// HTML is compared too, and it is not the interesting half: a bridge can lose a
// construct that renders to nothing, or swap one spelling for another, and the
// HTML is identical either way. That is the class carve-rs' own bridge gate was
// re-aimed for, and it is why the source comparison exists here as well.
//
// BOTH SIDES GO THROUGH `toCarve`. Comparing the bridge's output against
// `toCarve(source)` directly would put two different engine paths on the two
// sides of one equality - the canonical writer reads a parse of its own, while
// the bridge starts from the render parse - and two of the differences that
// turned up that way were the paths disagreeing rather than the bridge losing
// anything. `toCarve(back)` against `toCarve(source)` leaves the bridge as the
// only difference between the sides.
//
// THE LEDGER, and the direction that gives it teeth. `prosemirror-ledger.json`
// names three populations: the documents the bridge REPORTS as lossy (with the
// node types it reported), the documents it REFUSES, and the documents whose
// canonical source does not survive. All three are asserted as exact sets, so a
// document that starts diverging fails AND a document that stops diverging
// fails. A recorded loss that outlives its cause is a claim this suite has
// stopped checking.
import assert from 'node:assert/strict'
import { readFileSync, existsSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { toCarve, toHtml, toProseMirror, fromProseMirror, packageUnderTest } from './engine.mjs'
import { CORPUS, names } from './corpus-source.mjs'

const LEDGER = fileURLToPath(new URL('./prosemirror-ledger.json', import.meta.url))
const REGENERATE = 'UPDATE_PROSEMIRROR_LEDGER=1 node --stack-size=4000 tests/prosemirror.mjs'

assert.ok(existsSync(LEDGER), `no ledger at ${LEDGER} - regenerate it: ${REGENERATE}`)
const ledger = JSON.parse(readFileSync(LEDGER, 'utf8'))

const source = (name) => readFileSync(join(CORPUS, name), 'utf8')

/** Every node type a ProseMirror document holds, counted into `into`. */
function collectTypes(node, into) {
  into.set(node.type, (into.get(node.type) ?? 0) + 1)
  for (const child of node.content ?? []) collectTypes(child, into)
}

/** What one document does through the bridge. */
function measure(name) {
  const src = source(name)
  let bridged
  try {
    bridged = toProseMirror(src)
  } catch (error) {
    return { name, outcome: 'refused', detail: `toProseMirror: ${error.message}` }
  }

  // A REPORTED document is a third outcome, not a divergence. The bridge saying
  // what the ProseMirror model could not hold is the feature working; folding
  // those into the diverging pile would hide the report going away.
  const causes = [
    ...Object.keys(bridged.dropped).map((type) => `dropped:${type}`),
    ...Object.keys(bridged.degraded).map((type) => `degraded:${type}`),
  ].sort()
  if (causes.length > 0) return { name, outcome: 'reported', causes }

  let back
  try {
    back = fromProseMirror(bridged.json)
  } catch (error) {
    return { name, outcome: 'refused', detail: `fromProseMirror: ${error.message}` }
  }
  const types = new Map()
  collectTypes(JSON.parse(bridged.json), types)
  return {
    name,
    outcome: 'strict',
    types,
    html: toHtml(back) === toHtml(src) ? null : { got: toHtml(back), want: toHtml(src) },
    carve: toCarve(back) === toCarve(src) ? null : { got: toCarve(back), want: toCarve(src) },
  }
}

const results = names.map(measure)
const by = (outcome) => results.filter((result) => result.outcome === outcome)

const strict = by('strict')
const reported = by('reported')
const refused = by('refused')
const htmlLossy = strict.filter((result) => result.html !== null)
const sourceLossy = strict.filter((result) => result.carve !== null)

if (process.env.UPDATE_PROSEMIRROR_LEDGER === '1') {
  writeFileSync(LEDGER, `${JSON.stringify({
    $comment: ledger.$comment,
    reported: Object.fromEntries(reported.map((result) => [result.name, result.causes])),
    refused: Object.fromEntries(refused.map((result) => [
      result.name,
      ledger.refused?.[result.name] ?? result.detail,
    ])),
    sourceLossy: Object.fromEntries(sourceLossy.map((result) => [
      result.name,
      ledger.sourceLossy?.[result.name] ?? 'REASON MISSING - say what the bridge changed',
    ])),
  }, null, 2)}\n`, 'utf8')
  console.log(
    `prosemirror: ledger rewritten - ${reported.length} reported, ${refused.length} refused, ` +
      `${sourceLossy.length} source-lossy`,
  )
  process.exit(0)
}

// A FLOOR ON THE POPULATION THAT SURVIVES STRICTLY, and it is the guard against
// the ledger itself. Every assertion below compares the run to a committed list,
// so regenerating that list silences all of them at once; the floor is the one
// number regeneration cannot move. A fraction rather than a count, so adding
// documents upstream does not make it stale, and set far below the reading of
// the day (1350 of 1695, 80%) rather than against it.
const surviving = strict.length / names.length
assert.ok(
  surviving >= 0.5,
  `only ${strict.length} of ${names.length} documents (${(surviving * 100).toFixed(1)}%) reach the ` +
    'bridge without being reported lossy. This is a floor on the MEASUREMENT, not on the ledger: ' +
    'it is what stops a mass regression from being cleared by regenerating the ledger.',
)

// AND THE BRIDGE HAS TO BE DOING WORK. A bridge that emitted `{"type":"doc"}`
// for everything, or wrapped every document in one paragraph of text, would make
// each comparison below compare two equally empty things. Counting the distinct
// node types it produces is what such a bridge cannot fake, and it does not move
// with the size of the corpus the way a node total would.
const vocabulary = new Map()
for (const result of strict) {
  for (const [type, count] of result.types) {
    vocabulary.set(type, (vocabulary.get(type) ?? 0) + count)
  }
}
assert.ok(
  vocabulary.size >= 20,
  `the bridge produced only ${vocabulary.size} distinct ProseMirror node types over ` +
    `${strict.length} documents: ${[...vocabulary.keys()].sort().join(', ')}. A bridge that ` +
    'flattened every document would still satisfy every comparison below.',
)

// DIRECTION ONE: something broke. Every unrecorded divergence, in each of the
// three populations, asserted separately - a combined check cannot say which
// one fired.
const namesOf = (list) => list.map((result) => result.name)

if (htmlLossy.length > 0) {
  const [first] = htmlLossy
  console.error(`--- ${first.name} ---`)
  console.error(`want: ${JSON.stringify(first.html.want.slice(0, 400))}`)
  console.error(`got:  ${JSON.stringify(first.html.got.slice(0, 400))}`)
}
assert.deepEqual(
  namesOf(htmlLossy), [],
  `${htmlLossy.length} document(s) render differently after the bridge round trip: ` +
    `${namesOf(htmlLossy).slice(0, 10).join(', ')}. This list has no ledger on purpose - a ` +
    'document that reports nothing lost and then renders differently is a defect, not a recorded ' +
    'limitation.',
)

const recordedReported = ledger.reported ?? {}
const reportedChanges = reported
  .filter((result) => !recordedReported[result.name]
    || recordedReported[result.name].join(',') !== result.causes.join(','))
  .map((result) => `${result.name}: recorded ${JSON.stringify(recordedReported[result.name] ?? null)}, `
    + `now ${JSON.stringify(result.causes)}`)
assert.deepEqual(
  reportedChanges, [],
  `${reportedChanges.length} document(s) report a different set of unbridgeable node types than ` +
    `the ledger records: ${reportedChanges.slice(0, 5).join('; ')}. The ledger records WHAT the ` +
    'bridge could not hold, so a new cause and a known one are not the same entry.',
)

const unrecordedRefusals = namesOf(refused).filter((name) => !(ledger.refused ?? {})[name])
assert.deepEqual(
  unrecordedRefusals, [],
  `the bridge REFUSED ${unrecordedRefusals.length} document(s) the ledger does not name: ` +
    `${unrecordedRefusals.slice(0, 10).join(', ')}. A refusal is the schema map declining a ` +
    'payload it has never seen; it is recorded deliberately, never absorbed into another pile.',
)

const changedRefusals = refused
  .filter((result) => ledger.refused?.[result.name] && ledger.refused[result.name] !== result.detail)
  .map((result) => `${result.name}: recorded ${JSON.stringify(ledger.refused[result.name])}, `
    + `now ${JSON.stringify(result.detail)}`)
assert.deepEqual(
  changedRefusals, [],
  `${changedRefusals.length} recorded refusal(s) now fail for a different reason: ` +
    `${changedRefusals.join('; ')}.`,
)

const recordedLossy = ledger.sourceLossy ?? {}
const unrecordedLossy = namesOf(sourceLossy).filter((name) => !recordedLossy[name])
if (unrecordedLossy.length > 0) {
  const [first] = sourceLossy.filter((result) => result.name === unrecordedLossy[0])
  console.error(`--- ${first.name} ---`)
  console.error(`want: ${JSON.stringify(first.carve.want.slice(0, 400))}`)
  console.error(`got:  ${JSON.stringify(first.carve.got.slice(0, 400))}`)
}
assert.deepEqual(
  unrecordedLossy, [],
  `${unrecordedLossy.length} document(s) no longer write back the same canonical source and are ` +
    `not in the ledger: ${unrecordedLossy.slice(0, 10).join(', ')}. Either the bridge lost ` +
    `something it used to keep, or the loss is expected and belongs in ${LEDGER} with a reason.`,
)

// DIRECTION TWO: something was FIXED and the record outlived it. Without this
// the ledger is a skip list, and a skip list only ever grows.
const staleLossy = Object.keys(recordedLossy)
  .filter((name) => !namesOf(sourceLossy).includes(name))
  .filter((name) => names.includes(name))
assert.deepEqual(
  staleLossy, [],
  `${staleLossy.length} ledger entr(y|ies) now survive the bridge: ${staleLossy.slice(0, 10).join(', ')}. ` +
    `Delete them from ${LEDGER} - the allowlist has to shrink as the bridge improves, or it rots.`,
)

const staleReported = Object.keys(recordedReported)
  .filter((name) => !namesOf(reported).includes(name))
  .filter((name) => names.includes(name))
assert.deepEqual(
  staleReported, [],
  `${staleReported.length} ledger entr(y|ies) no longer report anything lost: ` +
    `${staleReported.slice(0, 10).join(', ')}. Delete them from ${LEDGER}.`,
)

const staleRefusals = Object.keys(ledger.refused ?? {})
  .filter((name) => !namesOf(refused).includes(name))
  .filter((name) => names.includes(name))
assert.deepEqual(
  staleRefusals, [],
  `${staleRefusals.length} ledger refusal(s) are no longer refused: ${staleRefusals.join(', ')}.`,
)

// A ledger entry naming a document the corpus no longer has is dead weight that
// makes the counts above lie about how much is recorded.
const present = new Set(names)
const phantom = [
  ...Object.keys(recordedReported),
  ...Object.keys(ledger.refused ?? {}),
  ...Object.keys(recordedLossy),
].filter((name) => !present.has(name))
assert.deepEqual(
  phantom, [],
  `${LEDGER} names documents the corpus does not have: ${phantom.slice(0, 10).join(', ')}`,
)

// EVERY RECORDED LOSS CARRIES A REASON. An entry whose reason is the placeholder
// the regeneration mode writes is an unexplained skip, which is the failure mode
// this ledger exists to avoid.
const unexplained = Object.entries(recordedLossy)
  .filter(([, reason]) => !reason || reason.startsWith('REASON MISSING'))
  .map(([name]) => name)
assert.deepEqual(
  unexplained, [],
  `${unexplained.length} ledger entr(y|ies) have no reason: ${unexplained.slice(0, 10).join(', ')}. ` +
    'Regenerating leaves a placeholder on purpose; say what the bridge changed.',
)

console.log(
  `prosemirror: ${strict.length}/${names.length} documents bridge with nothing reported lost, ` +
    `${sourceLossy.length} of those recorded source-lossy, ${reported.length} reported, ` +
    `${refused.length} refused, ${vocabulary.size} node types, at ${packageUnderTest}`,
)
