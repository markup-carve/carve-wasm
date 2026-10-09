// An independent census of the pairs the spec's example pages declare, ported
// from scripts/lib/example-pair-census.mjs in the spec repository.
//
// It lives in its own module so a test can call it with a synthetic page.
// tests/corpus-source.mjs runs its gate at import time and exits 0 when
// CARVE_SPEC_CORPUS is unset, so a counter reachable only through that file
// could never be watched go red - and a counter nobody has watched go red is
// the same dead check one layer up (markup-carve/carve#2824).
//
// A fence is tracked by its LITERAL opening backtick run, because inside a
// fence nothing is markup: a ```` example whose content is a ``` carve line
// declares no pair of its own.

const leadingRun = (text, character) => {
  let run = 0
  while (run < text.length && text[run] === character) run += 1
  return run
}

// `::: compare`, or a longer colon run, with optional modifiers such as
// `::: compare no-render`.
const COMPARE_OPEN = /^:{3,}[ \t]+compare([ \t]+\S.*)?$/

/**
 * @param {string[]} lines source lines of an example page
 * @returns {{blocks: {line: number, marker: string, carve: number, html: number, unclosed?: true}[], openFence: string | null}}
 *   one entry per `::: compare` block, in source order, plus the fence left open
 *   at the end of the page if there is one
 */
export const censusComparePairs = (lines) => {
  const blocks = []
  let block = null
  let fence = null

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index]

    if (fence !== null) {
      if (line.startsWith(fence) && line.slice(fence.length).trim() === '') fence = null
      continue
    }

    const ticks = leadingRun(line, '`')
    if (ticks >= 3) {
      fence = line.slice(0, ticks)
      const info = line.slice(ticks).trim()
      if (block !== null && info === 'carve') block.carve += 1
      if (block !== null && info === 'html') block.html += 1
      continue
    }

    const trimmed = line.trim()
    if (leadingRun(trimmed, ':') < 3) continue
    if (block === null) {
      if (COMPARE_OPEN.test(trimmed)) {
        block = { line: index + 1, marker: trimmed.match(/^:{3,}/)[0], carve: 0, html: 0 }
      }
      continue
    }
    if (trimmed === block.marker) {
      blocks.push(block)
      block = null
    }
  }

  if (block !== null) blocks.push({ ...block, unclosed: true })
  return { blocks, openFence: fence }
}

/**
 * How many corpus pairs a page declares: one per `carve` fence inside a
 * `::: compare` block. A block may hold several, and a `carve` fence outside
 * every block declares none.
 *
 * @param {string[]} lines source lines of an example page
 * @returns {number}
 */
export const countDeclaredPairs = (lines) =>
  censusComparePairs(lines).blocks.reduce((total, block) => total + block.carve, 0)
