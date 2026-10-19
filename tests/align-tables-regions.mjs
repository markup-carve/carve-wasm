// Aligned table writing and include-region selection (markup-carve/carve#3333).
//
// The alignment rows are the spec's shared fixtures, read from the spec
// checkout CI already makes for the corpus job. The region cases carry the
// line ranges carve-rs's `select_include_region` returns for them.
import assert from 'node:assert/strict'
import { existsSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { expandIncludes, selectIncludeRegion, toCarve, toCarveWithOptions, toHtml } from './engine.mjs'

const corpus = process.env.CARVE_SPEC_CORPUS
const fixtures = process.env.CARVE_SPEC_TABLE_ALIGNMENT ??
  (corpus ? join(dirname(dirname(resolve(corpus))), 'resources/spec/table-alignment-fixtures.json') : null)

if (fixtures && existsSync(fixtures)) {
  const cases = JSON.parse(readFileSync(fixtures, 'utf8'))
  assert.ok(cases.length >= 13, `${cases.length} alignment fixtures`)
  for (const { name, canonical, aligned } of cases) {
    assert.equal(toCarveWithOptions(canonical, { alignTables: true }), aligned, name)
    assert.equal(toCarveWithOptions(aligned, { alignTables: true }), aligned, `${name} fixed point`)
    assert.equal(toCarveWithOptions(aligned, { alignTables: false }), canonical, `${name} off`)
    assert.equal(toCarve(aligned), canonical, `${name} default`)
    assert.equal(toHtml(aligned), toHtml(canonical), `${name} HTML`)
  }
  console.log(`table alignment: ${cases.length} fixtures byte-identical`)
} else if (process.env.CI) {
  assert.fail('table-alignment-fixtures.json not found: set CARVE_SPEC_CORPUS or CARVE_SPEC_TABLE_ALIGNMENT')
} else {
  console.log('table alignment: skipped (no spec checkout; set CARVE_SPEC_CORPUS)')
}

const file = [
  'intro',
  '%% #region setup',
  'a',
  'b',
  '%% #endregion',
  '%% #region empty',
  '%% #endregion',
  '// #region snippet',
  'x',
  '// #endregion',
  '/* #region block */',
  'y',
  '/* #endregion */',
  '',
].join('\n')

// The same table carve-go's align_region_test.go holds its port to; null
// means include-region-invalid.
const regions = [
  [file, 'setup', 'carve', { start: 3, end: 4 }],
  [file, 'setup', 'code', { start: 3, end: 4 }],
  [file, 'empty', 'carve', { start: 7, end: 6 }],
  [file, 'snippet', 'code', { start: 9, end: 9 }],
  [file, 'block', 'code', { start: 12, end: 12 }],
  [file, 'snippet', 'carve', null],
  [file, 'missing', 'code', null],
  ['%% #region a\nx\n', 'a', 'carve', null],
  ['%% #region a\n%% #endregion b\n', 'a', 'carve', null],
  ['%% #region a\n%% #endregion\n%% #region a\n%% #endregion\n', 'a', 'carve', null],
  ['%% #region a\r\nx\r\n%% #endregion a\r\n', 'a', 'carve', { start: 2, end: 2 }],
  ['/* #region a\nx\n/* #endregion */\n', 'a', 'code', null],
  ['#region a\nx\n#endregion\n', 'a', 'code', { start: 2, end: 2 }],
  ['%% #regional a\n%% #region a\nx\n%% #endregion\n', 'a', 'carve', { start: 3, end: 3 }],
  ['%% #region 1a\n%% #endregion\n%% #region a\n%% #endregion\n', 'a', 'carve', null],
  ['-- #region q\nSELECT 1;\n-- #endregion q\n', 'q', 'code', { start: 2, end: 2 }],
  ['<!-- #region h -->\n<p>\n<!-- #endregion -->\n', 'h', 'code', { start: 2, end: 2 }],
  ['%%% #region a\nx\n%%% #endregion\n', 'a', 'carve', null],
  ['  %%\t#region a\nx\n%% #endregion a  \n', 'a', 'carve', { start: 2, end: 2 }],
]
for (const [source, name, mode, expected] of regions) {
  const label = `${JSON.stringify(source)} ${name} ${mode}`
  if (expected) {
    assert.deepEqual({ ...selectIncludeRegion(source, name, mode) }, expected, label)
    continue
  }
  assert.throws(() => selectIncludeRegion(source, name, mode), (error) => {
    assert.equal(error.name, 'IncludeRegionError')
    assert.equal(error.rule, 'include-region-invalid')
    return true
  }, label)
}
assert.throws(() => selectIncludeRegion(file, 'setup', 'markdown'), TypeError)
console.log(`include regions: ${regions.length} cases`)

// A host that read a file and found ill-formed UTF-8 refuses it as non-text.
const nonText = expandIncludes('{{ bin.crv }}\n', { resolve: () => ({ denial: 'include-non-text' }) })
assert.deepEqual(nonText.warnings.map((w) => w.rule), ['include-non-text'])
assert.deepEqual(nonText.dependencies, [{ id: 'bin.crv', resolved: false, denial: null }])
assert.deepEqual(nonText.resolverErrors, [])
