const encoder = new TextEncoder()

function convert(source, offset, from, to) {
  if (!Number.isSafeInteger(offset) || offset < 0) throw new RangeError('offset must be a non-negative safe integer')
  const position = { utf16: 0, utf8: 0, codepoint: 0 }
  for (const character of source) {
    if (position[from] === offset) return position[to]
    position.utf16 += character.length
    position.utf8 += encoder.encode(character).length
    position.codepoint++
    if (position[from] > offset) throw new RangeError('offset splits a Unicode character')
  }
  if (position[from] !== offset) throw new RangeError('offset exceeds the source length')
  return position[to]
}
export const utf16ToUtf8 = (source, offset) => convert(source, offset, 'utf16', 'utf8')
export const codepointToUtf8 = (source, offset) => convert(source, offset, 'codepoint', 'utf8')
export const utf8ToUtf16 = (source, offset) => convert(source, offset, 'utf8', 'utf16')
