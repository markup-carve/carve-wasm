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
// records the documents the bridge REPORTS as lossy (with the node types it
// reported), the ones it REFUSES, and the ones whose round trip does not
// reproduce the original. Every population is an exact set, so a document that
// starts diverging fails AND a document that stops diverging fails. A recorded
// loss that outlives its cause is a claim this suite has stopped checking.
//
// AND EVERY DIVERGENCE IS RECORDED WITH ITS BYTES. A ledger that only names a
// document leaves what the document produces free to be anything, which is how
// 355 reported documents compared nothing at all (markup-carve/carve-wasm#138).
// The recorded bytes are what the bridge produces today and carry no opinion
// about whether that is right; what they buy is that a change to them cannot
// land without a reviewer approving the diff. The population that needs no
// entry - a document whose round trip reproduces the spec's own rendering - is
// still compared against the spec rather than against a recording.
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
    return { name, causes: [], refusal: `toProseMirror: ${error.message}` }
  }

  // A REPORTED document is a different outcome, not a divergence. The bridge
  // saying what the ProseMirror model could not hold is the feature working;
  // folding those into the diverging pile would hide the report going away.
  //
  // IT IS NOT A REASON TO SKIP THE ROUND TRIP, which is what this did until
  // markup-carve/carve-wasm#138: `reported` returned here and neither
  // `toHtml(back)` nor `toCarve(back)` was ever computed, so 355 of 1740
  // documents - 20.4% - compared nothing at all. A document that reports
  // `degraded:escaped_text` should still carry everything else through.
  const causes = [
    ...Object.keys(bridged.dropped).map((type) => `dropped:${type}`),
    ...Object.keys(bridged.degraded).map((type) => `degraded:${type}`),
  ].sort()

  const types = new Map()
  collectTypes(JSON.parse(bridged.json), types)

  let back
  try {
    back = fromProseMirror(bridged.json)
  } catch (error) {
    return { name, causes, types, refusal: `fromProseMirror: ${error.message}` }
  }
  return {
    name,
    causes,
    types,
    html: toHtml(back) === toHtml(src) ? null : { got: toHtml(back), want: toHtml(src) },
    carve: toCarve(back) === toCarve(src) ? null : { got: toCarve(back), want: toCarve(src) },
  }
}

const results = names.map(measure)

// A document belongs to `reported` or to `strict` by what the bridge SAID, and
// to `refused` by whether the round trip could be run at all. The first two
// partition the corpus; a refusal overlaps whichever it came from, because a
// reported document still has causes worth pinning even when the bridge then
// declines its own output.
const reported = results.filter((result) => result.causes.length > 0)
const strict = results.filter((result) => result.causes.length === 0 && !result.refusal)
const refused = results.filter((result) => result.refusal)
const htmlLossy = strict.filter((result) => result.html !== null)
const sourceLossy = strict.filter((result) => result.carve !== null)
const reportedHtmlLossy = reported.filter((result) => !result.refusal && result.html !== null)
const reportedSourceLossy = reported.filter((result) => !result.refusal && result.carve !== null)

if (process.env.UPDATE_PROSEMIRROR_LEDGER === '1') {
  writeFileSync(LEDGER, `${JSON.stringify({
    $comment: ledger.$comment,
    reported: Object.fromEntries(reported.map((result) => [result.name, result.causes])),
    refused: Object.fromEntries(refused.map((result) => [
      result.name,
      ledger.refused?.[result.name] ?? result.refusal,
    ])),
    sourceLossy: Object.fromEntries(sourceLossy.map((result) => [result.name, {
      reason: ledger.sourceLossy?.[result.name]?.reason
        ?? (typeof ledger.sourceLossy?.[result.name] === 'string' ? ledger.sourceLossy[result.name] : null)
        ?? 'REASON MISSING - say what the bridge changed',
      carve: result.carve.got,
    }])),
    reportedHtml: Object.fromEntries(reportedHtmlLossy.map((result) => [result.name, result.html.got])),
    reportedSource: Object.fromEntries(reportedSourceLossy.map((result) => [result.name, result.carve.got])),
  }, null, 2)}\n`, 'utf8')
  console.log(
    `prosemirror: ledger rewritten - ${reported.length} reported, ${refused.length} refused, ` +
      `${sourceLossy.length} source-lossy, ${reportedHtmlLossy.length} reported-html, ` +
      `${reportedSourceLossy.length} reported-source`,
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
const measured = results.filter((result) => result.types)
for (const result of measured) {
  for (const [type, count] of result.types) {
    vocabulary.set(type, (vocabulary.get(type) ?? 0) + count)
  }
}
assert.ok(
  vocabulary.size >= 20,
  `the bridge produced only ${vocabulary.size} distinct ProseMirror node types over ` +
    `${measured.length} documents: ${[...vocabulary.keys()].sort().join(', ')}. A bridge that ` +
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
  .filter((result) => ledger.refused?.[result.name] && ledger.refused[result.name] !== result.refusal)
  .map((result) => `${result.name}: recorded ${JSON.stringify(ledger.refused[result.name])}, `
    + `now ${JSON.stringify(result.refusal)}`)
assert.deepEqual(
  changedRefusals, [],
  `${changedRefusals.length} recorded refusal(s) now fail for a different reason: ` +
    `${changedRefusals.join('; ')}.`,
)

// EVERY RECORDED DIVERGENCE IS COMPARED AGAINST ITS RECORDED BYTES, in each of
// the three populations that have one. A name on its own says a document
// diverges and leaves what it diverges INTO free to be anything, which is the
// hole markup-carve/carve-wasm#137 measured on the sibling round trip and
// markup-carve/carve-wasm#138 measured here.
//
// The recorded bytes are what the bridge produces today, so they carry no
// opinion about whether that is right. What makes them worth having is that a
// change to them cannot land without a reviewer approving the diff, and that
// the population they cover is the one the spec's own golden cannot reach: a
// document whose bridge output matches `toHtml(source)` needs no entry at all
// and is compared against the spec instead.
const pinned = ({ label, rows, recorded, expected = (entry) => entry, side, hint }) => {
  const unrecorded = rows.filter((result) => !(result.name in recorded)).map((result) => result.name)
  if (unrecorded.length > 0) {
    const [first] = rows.filter((result) => result.name === unrecorded[0])
    console.error(`--- ${first.name} (${label}) ---`)
    console.error(`want: ${JSON.stringify(first[side].want.slice(0, 400))}`)
    console.error(`got:  ${JSON.stringify(first[side].got.slice(0, 400))}`)
  }
  assert.deepEqual(
    unrecorded, [],
    `${unrecorded.length} document(s) now ${label}, and the ledger does not name them: ` +
      `${unrecorded.slice(0, 10).join(', ')}. ${hint}`,
  )

  const moved = rows
    .filter((result) => expected(recorded[result.name]) !== result[side].got)
    .map((result) => result.name)
  if (moved.length > 0) {
    const [first] = rows.filter((result) => result.name === moved[0])
    console.error(`--- ${first.name} (${label}) ---`)
    console.error(`recorded: ${JSON.stringify(String(expected(recorded[first.name])).slice(0, 400))}`)
    console.error(`now:      ${JSON.stringify(first[side].got.slice(0, 400))}`)
  }
  assert.deepEqual(
    moved, [],
    `${moved.length} recorded document(s) ${label}, but not as the ledger records it: ` +
      `${moved.slice(0, 10).join(', ')}. The ledger records what each one PRODUCES, so a known ` +
      `divergence changing shape is not the same entry. Regenerate it: ${REGENERATE}`,
  )

  const stale = Object.keys(recorded)
    .filter((name) => !rows.some((result) => result.name === name))
    .filter((name) => names.includes(name))
  assert.deepEqual(
    stale, [],
    `${stale.length} ledger entr(y|ies) no longer ${label}: ${stale.slice(0, 10).join(', ')}. ` +
      `Delete them from ${LEDGER} - the allowlist has to shrink as the bridge improves, or it rots.`,
  )
}

const recordedLossy = ledger.sourceLossy ?? {}
pinned({
  label: 'write back a different canonical source',
  rows: sourceLossy,
  recorded: recordedLossy,
  expected: (entry) => entry?.carve,
  side: 'carve',
  hint: 'Either the bridge lost something it used to keep, or the loss is expected and belongs in '
    + `${LEDGER} with a reason and the source it now writes.`,
})

// THE REPORTED POPULATION, round-tripped like any other. Reporting a loss is a
// good reason not to demand source identity; it is not a reason to compare
// nothing, which is what returning early did for 355 documents.
pinned({
  label: 'render differently after the bridge round trip, having reported a loss',
  rows: reportedHtmlLossy,
  recorded: ledger.reportedHtml ?? {},
  side: 'html',
  hint: 'A reported document still has to carry everything it did not report through the bridge.',
})
pinned({
  label: 'write back a different canonical source, having reported a loss',
  rows: reportedSourceLossy,
  recorded: ledger.reportedSource ?? {},
  side: 'carve',
  hint: 'A reported document still has to carry everything it did not report through the bridge.',
})

// DIRECTION TWO for the populations `pinned` does not cover. Without this the
// ledger is a skip list, and a skip list only ever grows.
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
  ...Object.keys(ledger.reportedHtml ?? {}),
  ...Object.keys(ledger.reportedSource ?? {}),
].filter((name) => !present.has(name))
assert.deepEqual(
  phantom, [],
  `${LEDGER} names documents the corpus does not have: ${phantom.slice(0, 10).join(', ')}`,
)

// EVERY RECORDED LOSS CARRIES A REASON. An entry whose reason is the placeholder
// the regeneration mode writes is an unexplained skip, which is the failure mode
// this ledger exists to avoid.
const unexplained = Object.entries(recordedLossy)
  .filter(([, entry]) => !entry?.reason || entry.reason.startsWith('REASON MISSING'))
  .map(([name]) => name)
assert.deepEqual(
  unexplained, [],
  `${unexplained.length} ledger entr(y|ies) have no reason: ${unexplained.slice(0, 10).join(', ')}. ` +
    'Regenerating leaves a placeholder on purpose; say what the bridge changed.',
)

console.log(
  `prosemirror: ${strict.length}/${names.length} documents bridge with nothing reported lost, ` +
    `${sourceLossy.length} of those recorded source-lossy, ${reported.length} reported and ` +
    `round-tripped too (${reportedHtmlLossy.length} recorded html-lossy, ` +
    `${reportedSourceLossy.length} recorded source-lossy), ${refused.length} refused, ` +
    `${vocabulary.size} node types, at ${packageUnderTest}`,
)
