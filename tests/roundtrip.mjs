// The `roundtrip` HTML import mode, driven over HTML this package produced.
//
// `htmlToCarve` exports a `roundtrip` mode whose own documentation says it is
// "only safe for Carve-produced HTML", and until this file NOTHING in the test
// surface ever carved a document and read it back (markup-carve/carve-wasm#53).
// What existed was:
//
//   - tests/engine.mjs listed `htmlToCarve` among the names the package must
//     export. That is an export check.
//   - src/lib.rs's `html_import_report_is_json` called the engine on a
//     HAND-WRITTEN HTML string and asserted the report mentions
//     `attribute-dropped`. That is a diagnostic check, and the HTML in it came
//     from a human rather than from a renderer.
//
// So the mode whose entire justification is that its input came from a Carve
// renderer had no test that produced such input. A test that runs a mode and
// asserts nothing about what came out is worse than no test, because the badge
// says covered - the shape catalogued in markup-carve/carve#755.
//
// WHAT IS ASSERTED, and why it is HTML rather than source.
//
// `carveToHtml(htmlToCarve(carveToHtml(x)))` equals `carveToHtml(x)`, per
// document, byte for byte. Source identity is not reachable and should not be
// asked for: the import is a projection through an HTML DOM, so `/italic/  *b*`
// comes back with its double space collapsed and `<input disabled>` comes back
// as `disabled=""`. Those are not defects and pinning them as source equality
// would be pinning the projection's incidental spelling. HTML-equivalence is
// what pandoc-carve and carve-grammars both settled on for their bridges, for
// the same reason.
//
// THE LEDGER, and the direction that gives it teeth. 1341 documents split into
// three: most survive, some come back rendering differently, and one is refused
// outright. `roundtrip-ledger.json` names the second and third groups. A
// document that diverges and is NOT named fails - that is a regression. A named
// document that now survives ALSO fails - a recorded loss that outlives its
// cause is a claim this suite has stopped checking, the same ratchet
// carve-grammars' KNOWN_LEAKS and pandoc-carve's KNOWN_LOSSY use.
//
// AND THE MODE ITSELF HAS TO BE LOAD-BEARING. Every assertion above would hold
// just as well if `mode` were ignored and every import ran as `safe` - the
// argument is a string that crosses the wasm boundary and is mapped in
// `html_import_mode`, and a boundary that dropped it would be invisible here.
// So the last check measures `roundtrip` against `safe` on the same HTML and
// requires them to actually differ, on a floor of documents.
import assert from 'node:assert/strict'
import { readFileSync, existsSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

import { toHtml, htmlToCarve, packageUnderTest } from './engine.mjs'
import { CORPUS, names } from './corpus-source.mjs'

const LEDGER = fileURLToPath(new URL('./roundtrip-ledger.json', import.meta.url))
const REGENERATE = 'UPDATE_ROUNDTRIP_LEDGER=1 node --stack-size=4000 tests/roundtrip.mjs'

assert.ok(existsSync(LEDGER), `no ledger at ${LEDGER} - regenerate it: ${REGENERATE}`)
const ledger = JSON.parse(readFileSync(LEDGER, 'utf8'))

const source = (name) => readFileSync(join(CORPUS, name), 'utf8')

/** What one document does through the round trip. */
function measure(name) {
  const html = toHtml(source(name))
  let carve
  try {
    carve = htmlToCarve(html, 'roundtrip').value
  } catch (error) {
    // A REFUSAL is a third outcome, not a divergence. The engine declining a
    // document - the nesting cap is the case on record - is a defense working,
    // and swallowing it into the diverging pile would hide it going away.
    return { name, outcome: 'refused', detail: String(error) }
  }
  const again = toHtml(carve)
  return again === html
    ? { name, outcome: 'survives' }
    : { name, outcome: 'diverges', html, carve, again }
}

const results = names.map(measure)
const by = (outcome) => results.filter((result) => result.outcome === outcome)

const survives = by('survives')
const diverges = by('diverges')
const refused = by('refused')

if (process.env.UPDATE_ROUNDTRIP_LEDGER === '1') {
  const { writeFileSync } = await import('node:fs')
  writeFileSync(LEDGER, `${JSON.stringify({
    $comment: ledger.$comment,
    diverges: diverges.map((result) => result.name),
    refused: Object.fromEntries(refused.map((result) => [
      result.name,
      ledger.refused?.[result.name] ?? result.detail,
    ])),
  }, null, 2)}\n`, 'utf8')
  console.log(`roundtrip: ledger rewritten - ${diverges.length} diverging, ${refused.length} refused`)
  process.exit(0)
}

// A FLOOR ON THE POPULATION THAT SURVIVES, and it is the guard against the
// ledger itself. Every assertion below compares the run to a committed list, so
// regenerating that list silences all of them at once; the floor is the one
// number regeneration cannot move. It is written as a FRACTION rather than a
// count so that adding documents upstream does not make it stale, and it sits
// far below the reading of the day (829 of 1341, 62%) rather than against it -
// a threshold set at today's measurement fails on the next corpus addition and
// teaches people to raise it without looking.
const surviving = survives.length / names.length
assert.ok(
  surviving >= 0.4,
  `only ${survives.length} of ${names.length} documents (${(surviving * 100).toFixed(1)}%) survive the ` +
    'round trip. This is a floor on the MEASUREMENT, not on the ledger: it is what stops a mass ' +
    'regression from being cleared by regenerating the ledger, so it is not cleared that way either.',
)

const recorded = new Set(ledger.diverges)
const refusedNames = new Set(Object.keys(ledger.refused ?? {}))

// DIRECTION ONE: something broke.
const unrecorded = diverges.map((result) => result.name).filter((name) => !recorded.has(name))
if (unrecorded.length > 0) {
  const [first] = diverges.filter((result) => result.name === unrecorded[0])
  console.error(`--- ${first.name} ---`)
  console.error(`carve back: ${JSON.stringify(first.carve.slice(0, 400))}`)
  console.error(`first html: ${JSON.stringify(first.html.slice(0, 400))}`)
  console.error(`re-render:  ${JSON.stringify(first.again.slice(0, 400))}`)
}
assert.deepEqual(
  unrecorded, [],
  `${unrecorded.length} document(s) no longer survive the round trip and are not in the ledger: ` +
    `${unrecorded.slice(0, 10).join(', ')}. Either the import lost something it used to keep, or the ` +
    `loss is expected and belongs in ${LEDGER} with the reason in the commit message.`,
)

const unrecordedRefusals = refused.map((result) => result.name).filter((name) => !refusedNames.has(name))
assert.deepEqual(
  unrecordedRefusals, [],
  `the import REFUSED ${unrecordedRefusals.length} document(s) the ledger does not name: ` +
    `${unrecordedRefusals.slice(0, 10).join(', ')}. A refusal is a defense firing or a defect; ` +
    'either way it is recorded deliberately, never absorbed into the diverging pile.',
)

// DIRECTION TWO: something was FIXED and the record outlived it. Without this
// the ledger is a skip list, and a skip list only ever grows.
const stale = [...recorded].filter((name) => survives.some((result) => result.name === name))
assert.deepEqual(
  stale, [],
  `${stale.length} ledger entr(y|ies) now survive the round trip: ${stale.slice(0, 10).join(', ')}. ` +
    `Delete them from ${LEDGER} - a recorded loss that outlives its cause is a claim this suite has ` +
    'stopped checking.',
)

const staleRefusals = [...refusedNames].filter((name) => !refused.some((result) => result.name === name))
assert.deepEqual(
  staleRefusals, [],
  `${staleRefusals.length} ledger refusal(s) are no longer refused: ${staleRefusals.join(', ')}. ` +
    `Delete them from ${LEDGER}.`,
)

// A ledger entry naming a document the corpus no longer has is dead weight that
// makes the counts above lie about how much is recorded.
const present = new Set(names)
const phantom = [...recorded, ...refusedNames].filter((name) => !present.has(name))
assert.deepEqual(
  phantom, [],
  `${LEDGER} names documents the corpus does not have: ${phantom.slice(0, 10).join(', ')}`,
)

// AND THE MODE ARGUMENT HAS TO REACH THE ENGINE.
//
// Everything above holds unchanged if `mode` were dropped at the wasm boundary
// and every import silently ran as `safe`. That is not a hypothetical worry
// about this binding: the mode crosses as an `Option<String>` and is mapped in
// `html_import_mode`, so a wrong default or a lost argument is a one-line
// change nothing else here would see. Measured on the same HTML, the two modes
// disagree on 244 of these documents; the floor is a fraction of that, low
// enough to survive corpus growth and high enough that one lucky document
// cannot satisfy it.
const modeSensitive = names.filter((name) => {
  const html = toHtml(source(name))
  try {
    return htmlToCarve(html, 'roundtrip').value !== htmlToCarve(html, 'safe').value
  } catch {
    return false
  }
})
assert.ok(
  modeSensitive.length >= 50,
  `roundtrip and safe produced the same Carve for all but ${modeSensitive.length} of ${names.length} ` +
    'documents. The mode argument is not reaching the engine, or the two modes have converged - ' +
    'either way, every assertion above would pass with the mode ignored entirely.',
)

console.log(
  `roundtrip: ${survives.length}/${names.length} documents re-render byte-identically through ` +
    `htmlToCarve(..., 'roundtrip'), ${diverges.length} recorded lossy, ${refused.length} refused, ` +
    `${modeSensitive.length} mode-sensitive, at ${packageUnderTest}`,
)
