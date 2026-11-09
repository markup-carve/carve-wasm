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
 * @returns {{blocks: {line: number, marker: string, carve: number, html: number, modifiers: Set<string>, inputs: string[], unclosed?: true}[], openFence: string | null}}
 *   one entry per `::: compare` block, in source order, plus the fence left open
 *   at the end of the page if there is one
 */
export const censusComparePairs = (lines) => {
  const blocks = []
  let block = null
  let fence = null
  let fenceInfo = null
  let fenceLines = []

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index]

    if (fence !== null) {
      if (line.startsWith(fence) && line.slice(fence.length).trim() === '') {
        if (block !== null && fenceInfo === 'carve') {
          block.inputs.push(fenceLines.join('\n'))
        }
        fence = null
        fenceInfo = null
        fenceLines = []
      } else {
        fenceLines.push(line)
      }
      continue
    }

    const ticks = leadingRun(line, '`')
    if (ticks >= 3) {
      fence = line.slice(0, ticks)
      const info = line.slice(ticks).trim()
      fenceInfo = info
      if (block !== null && info === 'carve') block.carve += 1
      if (block !== null && info === 'html') block.html += 1
      continue
    }

    const trimmed = line.trim()
    if (leadingRun(trimmed, ':') < 3) continue
    if (block === null) {
      if (COMPARE_OPEN.test(trimmed)) {
        block = { line: index + 1, marker: trimmed.match(/^:{3,}/)[0], carve: 0, html: 0,
          modifiers: new Set(trimmed.split(/\s+/).slice(2)), inputs: [] }
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

// The spec writes one fixture per distinct input after these byte modifiers.
// Keep the authored-pair count above independent of this selection, so a lost
// fence still fails the census before a unique-input count is considered.
export const countUniqueInputs = (blocks) => {
  const inputs = new Set()
  for (const block of blocks) {
    for (const input of block.inputs) {
      let value = `${input}\n`
      if (block.modifiers.has('no-final-newline')) value = value.replace(/\n$/, '')
      if (block.modifiers.has('crlf')) value = value.replace(/\n/g, '\r\n')
      if (block.modifiers.has('cr')) value = value.replace(/\n/g, '\r')
      if (block.modifiers.has('bom')) value = `\ufeff${value}`
      if (block.modifiers.has('nul')) value = value.replace(/\u2400/g, '\u0000')
      inputs.add(value)
    }
  }
  return inputs.size
}
