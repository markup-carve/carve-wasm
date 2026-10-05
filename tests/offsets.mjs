import assert from 'node:assert/strict'
import { utf16ToUtf8, codepointToUtf8, utf8ToUtf16 } from '../examples/offsets.mjs'
for (const source of ['😀é\r\nx', '\uFEFFa\r\nb', 'e\u0301😀']) {
  let utf16 = 0, byte = 0, point = 0
  for (const character of source) {
    assert.equal(utf16ToUtf8(source, utf16), byte)
    assert.equal(codepointToUtf8(source, point), byte)
    assert.equal(utf8ToUtf16(source, byte), utf16)
    utf16 += character.length
    byte += Buffer.byteLength(character)
    point++
  }
  assert.equal(utf16ToUtf8(source, source.length), Buffer.byteLength(source))
  assert.equal(codepointToUtf8(source, point), Buffer.byteLength(source))
}
assert.throws(() => utf16ToUtf8('😀', 1), RangeError)
assert.throws(() => utf8ToUtf16('é', 1), RangeError)
assert.throws(() => codepointToUtf8('a', 2), RangeError)
assert.throws(() => codepointToUtf8('a', -1), RangeError)
console.log('offsets: UTF-16, UTF-8 and codepoint conversions handle emoji, combining marks, CRLF and BOM')
