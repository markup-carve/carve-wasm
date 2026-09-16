// Exercise the built WASM ARTIFACT through the JS API users actually load.
// `cargo test` only tests the native build of the wrapper; it never proves the
// wasm package works -- which is how a stale engine can pass CI (the embedded
// engine in carve-go did exactly that for months).
import assert from 'node:assert/strict'
import {
  astJsonToCarve,
  astJsonToHtml,
  applyProfile,
  applySourcePatch,
  createSourcePatch,
  htmlToCarve,
  fromBbcode,
  fromDjot,
  fromProseMirror,
  fromMarkdown,
  migrateBbcode,
  migrateDjot,
  lintAccessibility,
  lintCarve,
  lintCarveWithOptions,
  markdownToAstJson,
  needsReview,
  parseLocator,
  parseSourceLayoutJson,
  sanitizeSvg,
  stampCarve,
  parseJson,
  parseJsonWithOptions,
  readStamp,
  toAnsi,
  toAnsiWithOptions,
  toCarve,
  toCarveWithOptions,
  toHtml,
  toHtmlWithOptions,
  toHtmlWithRenderers,
  toHtmlWithReport,
  toMarkdown,
  toMarkdownWithOptions,
  toPlainText,
  toPlainTextWithOptions,
  toProseMirror,
  toCarvePatch,
  expandIncludes,
  parseSnapshot,
  reparse,
  createAstPatch,
  applyAstPatch,
  createReversibleAstPatch,
  applyReversibleAstPatch,
} from './engine.mjs'

const cases = [
  // Superscript/subscript are braced-only: a bare `^` / `,` is literal text.
  ['a ^2^ b', '<p>a ^2^ b</p>'],
  ['x{^2^}', '<p>x<sup>2</sup></p>'],
  ['H,2,O', '<p>H,2,O</p>'],
  ['H{,2,}O', '<p>H<sub>2</sub>O</p>'],
  // A symbol needs a leading word boundary, so these stay literal.
  ['a:b:c and 10:30: here', '<p>a:b:c and 10:30: here</p>'],
  ['mail me@example.com now', '<p>mail me@example.com now</p>'],
  // Core sanity.
  ['# Title', '<section id="Title">\n  <h1>Title</h1>\n</section>'],
  ['a *bold* b', '<p>a <strong>bold</strong> b</p>'],
]

let failed = 0
for (const [src, want] of cases) {
  const got = toHtml(src).trim()
  if (got !== want) {
    console.error(`FAIL ${JSON.stringify(src)}\n  got:  ${got}\n  want: ${want}`)
    failed++
  }
}
assert.equal(failed, 0, `${failed} wasm artifact case(s) failed`)
console.log(`wasm artifact: ${cases.length}/${cases.length} cases pass`)

const source = 'see → here'
const sourcePatch = createSourcePatch(source, 'see ⇒ here', 'quick-fix', 'arrow-style')
assert.equal(sourcePatch.sourceBytes, Buffer.byteLength(source))
assert.equal(sourcePatch.edits[0].kind, 'quick-fix')
assert.equal(applySourcePatch(source, sourcePatch), 'see ⇒ here')
assert.throws(() => applySourcePatch('stale', sourcePatch))
assert.equal(applySourcePatch('# Title   ', toCarvePatch('# Title   ')), '# Title\n')
for (const kind of ['formatting', 'syntax-migration', 'quick-fix', 'refactor']) {
  const wire = JSON.parse(JSON.stringify(createSourcePatch('a', 'b', kind, 'smoke-test')))
  assert.equal(wire.edits[0].kind, kind)
  assert.equal(applySourcePatch('a', wire), 'b')
}
assert.deepEqual(createSourcePatch('same', 'same', 'refactor', 'no-op').edits, [])
assert.throws(() => createSourcePatch('a', 'b', 'unknown', 'bad-kind'), TypeError)
assert.throws(() => createSourcePatch('a', 'b', 'refactor', ''), TypeError)
console.log('wasm artifact: source patches pass')

const checked = toHtmlWithReport('`x`{=latex}', false, 1)
assert.equal(checked.value, '<p></p>')
assert.deepEqual(checked.losses.map(({ code, format, target, nodeType }) => ({ code, format, target, nodeType })), [
  { code: 'raw-format-dropped', format: 'latex', target: 'html', nodeType: 'inline' },
])
assert.equal(checked.losses[0].pos.startLine, 1)
assert.throws(
  () => toHtmlWithReport('`x`{=latex}', true, 1),
  (error) => error.name === 'RenderLossError' && error.totalLosses === 1 && error.losses.length === 1,
)
console.log('wasm artifact: checked render losses pass')

// The `sections` option, through the artifact. `cargo test` covers the Rust
// renderers directly; only this proves the option survives the wasm-bindgen
// boundary, where a boolean has to cross as a JS value.
assert.equal(
  toHtmlWithOptions('# A\n\np\n', { sections: false }).trim(),
  '<h1 id="A">A</h1>\n<p>p</p>',
)
// Omitted, null and an empty object all mean "defaults", so a caller may pass a
// partially-filled object.
for (const opts of [undefined, null, {}, { sections: true }]) {
  assert.equal(
    toHtmlWithOptions('# A\n', opts).trim(),
    '<section id="A">\n  <h1>A</h1>\n</section>',
    `defaults expected for ${JSON.stringify(opts) ?? 'undefined'}`,
  )
}
// Composes with the other two fields rather than being exclusive with them.
const composed = toHtmlWithOptions('# A\n\n:rocket:\n', {
  sections: false,
  symbols: { rocket: '🚀' },
  full: true,
}).trim()
// `full: true` enables heading permalinks, so the <h1> carries an anchor after
// its text - the id must still be on the <h1> itself, which is where that
// anchor's own href points (markup-carve/carve-rs#379).
assert.ok(composed.startsWith('<h1 id="A">A '), composed)
assert.ok(composed.includes('href="#A"'), composed)
assert.ok(composed.includes('🚀'), composed)
assert.ok(!composed.includes('<section'), composed)
// A recognized key with the wrong TYPE throws rather than being coerced: JS
// truthiness would read the string "false" as true, the opposite of what was
// written. An UNrecognized key is ignored - config typos should not break a
// render.
assert.throws(() => toHtmlWithOptions('# A\n', { sections: 'false' }), TypeError)
// Same contract for `symbols`: a wrong type throws instead of quietly rendering
// without the map, which would lose the caller's configuration silently.
assert.throws(() => toHtmlWithOptions('# A\n', { symbols: 'rocket' }), TypeError)
assert.throws(() => toHtmlWithOptions('# A\n', { symbols: 1 }), TypeError)
assert.equal(
  toHtmlWithOptions('# A\n', { sctions: false }).trim(),
  '<section id="A">\n  <h1>A</h1>\n</section>',
)
console.log('wasm artifact: sections option passes')

// The `rawHtml` option, through the artifact. This is the switch that lets a
// host render a document it did not author: a passthrough is the one construct
// that can put author-controlled markup on the host's origin.
const RAW_SOURCE = '```=html\n<img src=x onerror=alert(1)>\n```\n\nan `<b>x</b>`{=html} span\n'
const rawOn = toHtmlWithOptions(RAW_SOURCE, {}).trim()
assert.ok(rawOn.includes('<img src=x onerror=alert(1)>'), rawOn)
assert.ok(rawOn.includes('<b>x</b>'), rawOn)
const rawOff = toHtmlWithOptions(RAW_SOURCE, { rawHtml: false }).trim()
assert.ok(rawOff.includes('&lt;img src=x onerror=alert(1)&gt;'), rawOff)
assert.ok(!rawOff.includes('<img src=x'), rawOff)
assert.ok(rawOff.includes('&lt;b&gt;x&lt;/b&gt;'), rawOff)
// Composes with the extension path, which builds its options separately, and
// leaves the symbol map's TRUSTED-RAW contract alone: `rawHtml` is about the
// document's passthrough, not about what the host configured.
const rawFull = toHtmlWithOptions('```=html\n<b>raw</b>\n```\n\n:bold:\n', {
  rawHtml: false,
  full: true,
  symbols: { bold: '<b>x</b>' },
}).trim()
assert.ok(rawFull.includes('&lt;b&gt;raw&lt;/b&gt;'), rawFull)
assert.ok(rawFull.includes('<b>x</b>'), rawFull)
// Same wrong-type contract as the other booleans.
assert.throws(() => toHtmlWithOptions('# A\n', { rawHtml: 'false' }), TypeError)
console.log('wasm artifact: rawHtml option passes')

// The profile, through the artifact. A rejection has to ARRIVE as an error: the
// engine's infallible entry point turns one into an empty string, which a
// caller cannot tell from a document that rendered to nothing.
const rejected = (() => {
  try {
    return { html: toHtmlWithOptions('| a | b |\n|---|---|\n| 1 | 2 |\n', { profile: 'minimal' }) }
  } catch (error) {
    return { error }
  }
})()
if (rejected.error) {
  assert.equal(rejected.error.name, 'ProfileViolationError')
  assert.ok(Array.isArray(rejected.error.violations), 'violations should be an array')
  assert.ok(rejected.error.violations.length > 0, 'violations should not be empty')
} else {
  assert.notEqual(rejected.html, '', 'a rejection must not arrive as an empty string')
}
assert.ok(toHtmlWithOptions('# A\n', { profile: 'full' }).includes('<h1'))
assert.throws(() => toHtmlWithOptions('# A\n', { profile: 'nope' }), TypeError)

// Editor-preview switches.
assert.ok(!toHtmlWithOptions('# A\n\np\n', {}).includes('data-source-line'))
assert.ok(toHtmlWithOptions('# A\n\np\n', { sourceLine: true }).includes('data-source-line'))

// Output-shaping switches: the i18n seam, typography, and the slug policy.
assert.ok(toHtmlWithOptions('::: note\nbody\n:::\n', {}).includes('Note'))
assert.ok(
  toHtmlWithOptions('::: note\nbody\n:::\n', {
    labels: { admonitionNote: 'Hinweis' },
  }).includes('Hinweis'),
)
assert.ok(toHtmlWithOptions('a...b\n', { smartTypography: 'source' }).includes('a...b'))
assert.ok(
  toHtmlWithOptions('# Grüße Alle\n', {
    lowercaseHeadingIds: true,
    asciiHeadingIds: 'strict',
  }).includes('id="grusse-alle"'),
)
// Social-token URL templates. Without them a mention and a tag are inert spans,
// which is the whole reason a host has to be able to say where they point.
{
  const bare = toHtmlWithOptions('Hi @ada and #rust\n', {})
  assert.ok(!bare.includes('<a'), 'mentions and tags are inert without a template')
  const linked = toHtmlWithOptions('Hi @ada and #rust\n', {
    mentionUrl: 'https://example.com/users/{name}',
    tagUrl: 'https://example.com/tags/{name}',
  })
  assert.ok(linked.includes('href="https://example.com/users/ada"'), linked)
  assert.ok(linked.includes('href="https://example.com/tags/rust"'), linked)
  // The name is the document's, not the host's: it is encoded into the
  // template rather than concatenated onto it.
  assert.ok(
    toHtmlWithOptions('@a b\n', { mentionUrl: 'https://example.com/{user}' }).includes(
      'href="https://example.com/a"',
    ),
  )
}
assert.throws(() => toHtmlWithOptions('# A\n', { mentionUrl: 7 }), TypeError)
assert.throws(() => toHtmlWithOptions('# A\n', { tagUrl: 7 }), TypeError)

// `mode: 'static'` is accepted and renders; what it flattens is the engine's
// business, covered there.
assert.ok(toHtmlWithOptions('# A\n', { mode: 'static' }).includes('<h1'))
assert.throws(() => toHtmlWithOptions('# A\n', { mode: 'nope' }), TypeError)
assert.throws(() => toHtmlWithOptions('# A\n', { labels: 'Hinweis' }), TypeError)
assert.throws(() => toHtmlWithOptions('# A\n', { sourceLine: 'yes' }), TypeError)
console.log('wasm artifact: profile, editor and output-shaping options pass')

// The options object on the other four targets and on the tree export
// (markup-carve/carve-wasm#108). Before this the same package rendered one
// document safely to HTML and unsafely to Markdown, because `profile` had
// nowhere to be passed.
//
// Each target is asserted on its own. A loop would report that SOMETHING lost
// the profile, and which target lost it is the whole content.
{
  const DENIED = '# Heading\n\n![alt](x.png)\n'
  const COMMENT = { profile: 'comment' }

  // No options at all is the target's own entry point, unchanged.
  assert.equal(toMarkdownWithOptions(DENIED), toMarkdown(DENIED))
  assert.equal(toPlainTextWithOptions(DENIED), toPlainText(DENIED))
  assert.equal(toAnsiWithOptions(DENIED), toAnsi(DENIED))
  assert.equal(toCarveWithOptions(DENIED), toCarve(DENIED))
  assert.equal(parseJsonWithOptions(DENIED), parseJson(DENIED))
  // An object that says nothing must not cost the positions PART 12 §4
  // requires the serialized form to carry.
  assert.equal(parseJsonWithOptions(DENIED, {}), parseJson(DENIED))
  assert.ok(parseJson(DENIED).includes('"startOffset"'))

  // Under `comment` a heading is not a heading and an image is not an image.
  assert.equal(toMarkdownWithOptions(DENIED, COMMENT), '# Heading\n\n[img: alt\\]\n')
  assert.equal(toPlainTextWithOptions(DENIED, COMMENT), '# Heading\n\n[img: alt]\n')
  assert.equal(toAnsiWithOptions(DENIED, COMMENT), '# Heading\n\n[img: alt]\n')
  assert.equal(toCarveWithOptions(DENIED, COMMENT), '\\# Heading\n\n[img: alt]\n')
  assert.ok(!parseJsonWithOptions(DENIED, COMMENT).includes('"heading"'))
  assert.ok(parseJson(DENIED).includes('"heading"'))

  // The length bound is the one profile rule that refuses outright, and a
  // rejection has to ARRIVE as an error rather than as an empty string.
  const long = 'a'.repeat(16 * 1024)
  for (const [name, render] of [
    ['toMarkdownWithOptions', toMarkdownWithOptions],
    ['toPlainTextWithOptions', toPlainTextWithOptions],
    ['toAnsiWithOptions', toAnsiWithOptions],
    ['toCarveWithOptions', toCarveWithOptions],
    ['parseJsonWithOptions', parseJsonWithOptions],
  ]) {
    let error
    try {
      render(long, { profile: 'minimal' })
    } catch (caught) {
      error = caught
    }
    assert.ok(error, `${name} must refuse a document past the profile's length bound`)
    assert.equal(error.name, 'ProfileViolationError', name)
    assert.match(error.violations[0], /maximum length/, name)
  }

  // `smartTypography` is the other option these targets read.
  assert.equal(toMarkdownWithOptions('a ... b\n', { smartTypography: 'source' }), 'a ... b\n')
  assert.equal(toMarkdownWithOptions('a ... b\n', {}), 'a … b\n')

  // The Carve writer reads `profile` and nothing else, because the engine's
  // canonical writer is parse-only by contract. It is accepted rather than
  // refused so one options object can be handed to every target, which is the
  // whole point - but the README says so, so pin it.
  const NUMBERED = '# One\n\n## Two\n'
  const NUMBERS = { extensions: ['heading-numbers'] }
  assert.equal(toMarkdownWithOptions(NUMBERED, NUMBERS), '# 1 One\n\n## 1.1 Two\n')
  assert.equal(toPlainTextWithOptions(NUMBERED, NUMBERS), '1 One\n\n1.1 Two\n')
  assert.equal(toCarveWithOptions(NUMBERED, NUMBERS), toCarveWithOptions(NUMBERED, {}))
  assert.equal(toCarveWithOptions('a ... b\n', { smartTypography: 'source' }), 'a ... b\n')
  assert.equal(toCarveWithOptions('a ... b\n', {}), 'a ... b\n')

  // The one reader, so the same object gets the same contract everywhere: a
  // recognized key with the wrong type throws, and `renderers` is refused
  // because a bare string has nowhere to report a callback failure.
  assert.throws(() => toMarkdownWithOptions('# A\n', { sections: 'false' }), TypeError)
  assert.throws(() => toPlainTextWithOptions('# A\n', { profile: 'nope' }), TypeError)
  assert.throws(() => toAnsiWithOptions('# A\n', { symbols: 'rocket' }), TypeError)
  assert.throws(
    () => toCarveWithOptions('# A\n', { renderers: { math: () => '' } }),
    TypeError,
  )
  assert.throws(() => parseJsonWithOptions('# A\n', { labels: 'Hinweis' }), TypeError)
}
console.log('wasm artifact: the options object reaches every render target')

// The static math renderer (markup-carve/carve-wasm#94).
//
// Each case is driven on its own rather than through a loop that ORs them: the
// distinction between "the callback was never called", "it was called and threw"
// and "it was called and returned the wrong type" is the whole content here, and
// a combined assertion cannot say which one fired.
{
  const MATH = '``` math\nE = mc^2\n```\n'
  const STATIC = { mode: 'static', extensions: ['math-block'] }
  const run = (math) => toHtmlWithRenderers(MATH, { ...STATIC, renderers: { math } })

  // The callback reaches the engine, with both of its arguments. `display` is
  // asserted because a binding that dropped it would still produce output.
  const seen = []
  const ok = run((tex, display) => {
    seen.push([tex, display])
    return `<math>${tex}</math>`
  })
  assert.deepEqual(seen, [['E = mc^2', true]])
  assert.equal(ok.html, '<div class="math display"><math>E = mc^2</math></div>')
  assert.deepEqual(ok.rendererErrors, [])

  // TRUSTED RAW: what the callback returns is not escaped. This is the
  // documented contract, so it is pinned rather than left to be discovered.
  assert.ok(run(() => '<script>x</script>').html.includes('<script>x</script>'))

  // Without a renderer the static path keeps the source - never blank.
  const bare = toHtmlWithRenderers(MATH, STATIC)
  assert.equal(bare.html, '<div class="math display">\\[E = mc^2\\]</div>')
  assert.deepEqual(bare.rendererErrors, [])

  // A THROW is recorded and reported, not swallowed and not propagated: the
  // engine's closure returns a String and cannot unwind.
  const threw = run(() => {
    throw new Error('katex blew up')
  })
  assert.equal(threw.html, '<div class="math display"></div>')
  assert.equal(threw.rendererErrors.length, 1)
  assert.equal(threw.rendererErrors[0].renderer, 'math')
  assert.equal(threw.rendererErrors[0].display, true)
  assert.equal(threw.rendererErrors[0].source, 'E = mc^2')
  assert.match(threw.rendererErrors[0].message, /katex blew up/)

  // A thrown non-Error still carries its text.
  assert.match(
    run(() => {
      throw 'plain string'
    }).rendererErrors[0].message,
    /plain string/,
  )

  // THE PROMISE CASE. An `async` renderer returns a Promise; stringifying it
  // would put `[object Promise]` in the document, so it is reported as a
  // failure and the node emits nothing.
  const promised = run(async (tex) => `<math>${tex}</math>`)
  assert.equal(promised.html, '<div class="math display"></div>')
  assert.match(promised.rendererErrors[0].message, /Promise/)
  assert.ok(!promised.html.includes('Promise'), promised.html)

  // Any other wrong type is reported too, named by its typeof.
  assert.match(run(() => 7).rendererErrors[0].message, /number/)
  assert.match(run(() => undefined).rendererErrors[0].message, /undefined/)

  // Two failing formulas report twice. A single cell that only remembers the
  // last failure would pass every assertion above.
  const twice = toHtmlWithRenderers('``` math\na\n```\n\n``` math\nb\n```\n', {
    ...STATIC,
    renderers: {
      math: (tex) => {
        throw new Error(`no ${tex}`)
      },
    },
  })
  assert.deepEqual(
    twice.rendererErrors.map((e) => e.source),
    ['a', 'b'],
  )

  // READ-TIME validation, consistent with every other field on the object.
  assert.throws(() => run('nope'), TypeError)
  assert.throws(() => run(42), TypeError)
  assert.throws(() => toHtmlWithRenderers(MATH, { ...STATIC, renderers: 'nope' }), TypeError)
  // A read-time throw does not need a document that contains a formula.
  assert.throws(() => toHtmlWithRenderers('plain\n', { renderers: { math: 'nope' } }), TypeError)

  // `toHtmlWithOptions` refuses `renderers` outright - it returns a bare
  // string, so a reported failure would have nowhere to go.
  assert.throws(
    () => toHtmlWithOptions(MATH, { ...STATIC, renderers: { math: () => 'x' } }),
    TypeError,
  )

  // The rest of the options object still applies through this entry point. A
  // binding that read `renderers` and forgot everything else would pass above.
  assert.ok(toHtmlWithRenderers('# A\n\np\n', { sections: false }).html.startsWith('<h1'))
  assert.throws(() => toHtmlWithRenderers('# A\n', { sections: 'false' }), TypeError)
  const shaped = { profile: 'minimal', smartTypography: 'source', labels: { note: 'Hinweis' } }
  assert.equal(toHtmlWithRenderers(RAW_SOURCE, shaped).html, toHtmlWithOptions(RAW_SOURCE, shaped))
  assert.equal(toHtmlWithRenderers('# A\n').html, toHtml('# A\n'))
}
console.log('wasm artifact: static math renderer cases pass')

// The static diagram renderers (markup-carve/carve-wasm#105).
//
// Driven case by case for the reason the math block above is: "never called",
// "called and threw" and "called and returned the wrong type" are the content.
{
  const MERMAID = '``` mermaid\ngraph TD; A-->B\n```\n'
  const STATIC = { mode: 'static', extensions: ['fenced-render'] }
  const run = (mermaid) =>
    toHtmlWithRenderers(MERMAID, { ...STATIC, renderers: { diagrams: { mermaid } } })

  // The callback reaches the engine with the fence's source, and what it
  // returns is TRUSTED RAW inside the class-carrying wrapper.
  const seen = []
  const ok = run((source) => {
    seen.push(source)
    return `<svg>${source}</svg>`
  })
  assert.deepEqual(seen, ['graph TD; A-->B'])
  assert.equal(
    ok.html,
    '<div class="mermaid" role="img" aria-label="mermaid"><svg>graph TD; A-->B</svg></div>',
  )
  assert.deepEqual(ok.rendererErrors, [])

  // The shape the ruling picked: a lookup is the callback in one line. This is
  // the case a browser host actually has, Mermaid's own `render` being a
  // Promise from v10 on.
  const prerendered = new Map([['graph TD; A-->B', '<svg id="pre"/>']])
  assert.ok(run((source) => prerendered.get(source)).html.includes('<svg id="pre"/>'))

  // Without a renderer the static path degrades to an escaped source block -
  // never blank, and nothing to report.
  const bare = toHtmlWithRenderers(MERMAID, STATIC)
  assert.equal(
    bare.html,
    '<pre class="mermaid"><code class="language-mermaid">graph TD; A--&gt;B\n</code></pre>',
  )
  assert.deepEqual(bare.rendererErrors, [])

  // A throw is recorded under the FENCE CLASS, not under `math`.
  const threw = run(() => {
    throw new Error('mermaid blew up')
  })
  assert.equal(threw.html, '<div class="mermaid" role="img" aria-label="mermaid"></div>')
  assert.equal(threw.rendererErrors.length, 1)
  assert.equal(threw.rendererErrors[0].renderer, 'mermaid')
  assert.equal(threw.rendererErrors[0].source, 'graph TD; A-->B')
  assert.match(threw.rendererErrors[0].message, /mermaid blew up/)
  // `display` is the math callback's second argument. A diagram has no such
  // flag, so the key is absent rather than a made-up `false`.
  assert.ok(!('display' in threw.rendererErrors[0]), Object.keys(threw.rendererErrors[0]).join())

  // An `async` renderer is the mistake this binding exists to report, since
  // the field's name is exactly what a host would hand Mermaid to.
  const promised = run(async (source) => `<svg>${source}</svg>`)
  assert.equal(promised.html, '<div class="mermaid" role="img" aria-label="mermaid"></div>')
  assert.match(promised.rendererErrors[0].message, /Promise/)
  assert.ok(!promised.html.includes('Promise'), promised.html)
  assert.match(run(() => 7).rendererErrors[0].message, /number/)

  // Two keys are two callbacks. One cell holding the last-registered callback
  // would pass every assertion above.
  const two = toHtmlWithRenderers('``` mermaid\nm\n```\n\n``` dot\nd\n```\n', {
    mode: 'static',
    extensions: ['fenced-render', 'fenced-render-graphviz'],
    renderers: { diagrams: { mermaid: (s) => `<M>${s}</M>`, graphviz: (s) => `<G>${s}</G>` } },
  })
  assert.ok(two.html.includes('<M>m</M>'), two.html)
  assert.ok(two.html.includes('<G>d</G>'), two.html)

  // `math` and `diagrams` in one call, because the set is read once and both
  // halves have to survive it.
  const mixed = toHtmlWithRenderers('``` math\nE\n```\n\n``` mermaid\nm\n```\n', {
    mode: 'static',
    extensions: ['math-block', 'fenced-render'],
    renderers: { math: (tex) => `<K>${tex}</K>`, diagrams: { mermaid: (s) => `<M>${s}</M>` } },
  })
  assert.ok(mixed.html.includes('<K>E</K>'), mixed.html)
  assert.ok(mixed.html.includes('<M>m</M>'), mixed.html)

  // A MISSING KEY reports nothing, and this is the measurement behind saying
  // so: a `mermaid` fence under a `graphviz`-only configuration degrades to
  // source, and the binding is never called, so it cannot see the miss.
  const missed = toHtmlWithRenderers(MERMAID, {
    ...STATIC,
    renderers: { diagrams: { graphviz: (s) => `<G>${s}</G>` } },
  })
  assert.equal(missed.html, bare.html)
  assert.deepEqual(missed.rendererErrors, [])

  // A configured key the document never uses is silent too.
  assert.deepEqual(
    toHtmlWithRenderers('plain\n', { ...STATIC, renderers: { diagrams: { mermaid: (s) => s } } })
      .rendererErrors,
    [],
  )

  // READ-TIME validation is SHAPE ONLY. An unknown key is accepted, because
  // checking it against the fence classes would mean keeping that list by hand
  // here until markup-carve/carve-rs#1670 puts it on the registry entry.
  assert.deepEqual(
    toHtmlWithRenderers(MERMAID, { ...STATIC, renderers: { diagrams: { nosuch: (s) => s } } })
      .rendererErrors,
    [],
  )
  assert.throws(() => run('nope'), TypeError)
  assert.throws(() => run(42), TypeError)
  assert.throws(
    () => toHtmlWithRenderers(MERMAID, { ...STATIC, renderers: { diagrams: 'nope' } }),
    TypeError,
  )
  // A bare function is the shape a host reaches for after reading
  // `renderers.math`, so the error names the keyed spelling instead.
  assert.throws(
    () => toHtmlWithRenderers(MERMAID, { ...STATIC, renderers: { diagrams: () => 'x' } }),
    /keyed by the fence's css class/,
  )
  // A read-time throw does not need a document that contains a diagram.
  assert.throws(
    () => toHtmlWithRenderers('plain\n', { renderers: { diagrams: { mermaid: 'nope' } } }),
    TypeError,
  )
}
console.log('wasm artifact: static diagram renderer cases pass')

// The round trip. `parseJson` writes a tree out; until `astJsonToHtml` there was
// no way to render an edited one back, which is the reason to read a tree in a
// browser at all.
const treeSource = '# Title\n\nBody with _emphasis_.\n'
const tree = parseJson(treeSource)
assert.ok(astJsonToHtml(tree).includes('<h1'), 'a tree should render')
assert.equal(astJsonToCarve(tree).trim(), treeSource.trim())
// The options object reaches the tree path too, so an edited tree renders under
// the switches the host already configured.
assert.ok(!astJsonToHtml(tree, { sections: false }).includes('<section'))
assert.throws(() => astJsonToHtml('{"type":"nope"}'), Error)

// One options object has to mean the same thing on both paths. The tree
// renderer applies neither the profile filter nor the before-render hooks by
// itself, so rendering a tree straight through it made `profile` silently
// inert and `full` silently do nothing - the same call, two safety postures.
const parityDoc = '# H\n\ntext\n'
assert.equal(
  astJsonToHtml(parseJson(parityDoc), { full: true }),
  toHtmlWithOptions(parityDoc, { full: true }),
)
const parityTable = '| a | b |\n|---|---|\n| 1 | 2 |\n'
const treeFiltered = astJsonToHtml(parseJson(parityTable), { profile: 'minimal' })
assert.equal(treeFiltered, toHtmlWithOptions(parityTable, { profile: 'minimal' }))
assert.ok(!treeFiltered.includes('<table'), 'the profile has to filter on the tree path too')

// The filter on its own, with the tree kept. Until now a host could only get a
// profile applied on the way to HTML, and the HTML path discards what the filter
// did.
const denied = parseJson('a [home](https://example.com/x) b\n')
const stripped = applyProfile(denied, 'minimal')
assert.deepEqual(
  stripped.violations.map(({ nodeType, reason }) => ({ nodeType, reason })),
  [{ nodeType: 'link', reason: 'element_not_allowed' }],
)
assert.equal(
  stripped.violations[0].reasonDescription,
  'Links are disabled in this minimal context.',
)
assert.equal(
  stripped.violations[0].message,
  "'link' is not allowed: element_not_allowed (Links are disabled in this minimal context.)",
)
// A tree, not prose: the result feeds straight back into the entry points that
// take one.
assert.ok(!astJsonToHtml(stripped.json).includes('<a '), 'the link should be gone')
assert.equal(astJsonToCarve(stripped.json).trim(), 'a home b')

// A profile that denies nothing in this document reports nothing and changes
// nothing.
assert.deepEqual(applyProfile(denied, 'full').violations, [])
assert.equal(astJsonToHtml(applyProfile(denied, 'full').json), astJsonToHtml(denied))

// max_nesting is the filter's own bound, and it reports under a node type the
// document never denied.
const deep = applyProfile(parseJson('- a\n  - b\n    - c\n      - d\n'), 'minimal')
assert.ok(
  deep.violations.some(({ reason }) => reason === 'max_nesting_exceeded'),
  'minimal caps nesting at 2',
)

// The typography switch reaches the degrade. `to_text` decides the spelling of
// the text it substitutes, so a caller asking for the author's run has to get
// the author's run out of the FILTER, not only out of the renderer.
const dashed = parseJson('[x -- y](https://example.com/x)\n')
assert.equal(astJsonToHtml(applyProfile(dashed, 'minimal').json), '<p>x – y</p>')
assert.equal(
  astJsonToHtml(applyProfile(dashed, 'minimal', { smartTypography: 'source' }).json),
  '<p>x -- y</p>',
)

// Wrong types throw rather than being coerced, the way the render options do.
assert.throws(() => applyProfile(denied, 'nope'), TypeError)
assert.throws(() => applyProfile(denied, 'minimal', { smartTypography: 'nope' }), TypeError)
assert.throws(() => applyProfile(denied, 'minimal', { profileBaseHost: 7 }), TypeError)
assert.throws(() => applyProfile('{', 'full'), Error)

// The linter, with the rule ids carve-js and carve-php share.
assert.deepEqual(lintCarve('# Fine\n\ntext\n'), [])
const warnings = lintCarve('# Heading\n\n{.orphan}\n')
assert.equal(warnings.length, 1, 'a block attribute reaching no block should warn')
assert.equal(warnings[0].rule, 'unattached-block-attribute')
for (const key of ['line', 'column', 'rule', 'message', 'start', 'end']) {
  assert.ok(key in warnings[0], `a warning should carry ${key}`)
}

// The second diagnostic family, with its own rule ids and its own severity.
assert.deepEqual(lintAccessibility('# One\n\n![alt](x.png)\n'), [])
{
  const missing = lintAccessibility('![](x.png)\n')
  assert.equal(missing.length, 1)
  assert.equal(missing[0].rule, 'a11y/image-alt')
  assert.equal(missing[0].severity, 'error')
  assert.equal(typeof missing[0].startOffset, 'number')

  const jump = lintAccessibility('# One\n\n### Three\n')
  assert.equal(jump.length, 1)
  assert.equal(jump[0].rule, 'a11y/heading-jump')
  assert.equal(jump[0].severity, 'warning')
}

// Which degradations exist depends on the extensions in play, so the linter
// needs the set the host renders with. `samp` becomes an element only under
// `semantic-span`, and only then is a value on it discarded.
{
  const src = 'a [x]{samp=out} b\n'
  assert.deepEqual(lintCarve(src), [])
  assert.deepEqual(lintCarveWithOptions(src), [])
  assert.deepEqual(lintCarveWithOptions(src, {}), [])
  const extended = lintCarveWithOptions(src, { extensions: ['semantic-span'] })
  assert.equal(extended.length, 1)
  assert.equal(extended[0].rule, 'semantic-attribute-value-ignored')
  assert.throws(() => lintCarveWithOptions(src, { extensions: ['nope'] }), TypeError)
}

// Provenance. `readStamp` could read a marker this package could not write.
assert.equal(readStamp('# Plain\n'), null)
assert.equal(needsReview('# Plain\n', '1.0.0'), true)
{
  const stamped = stampCarve('# Plain\n', 'my-app 1.2')
  const read = readStamp(stamped)
  assert.equal(read.generatedBy, 'my-app 1.2')
  assert.equal(needsReview(stamped, read.version), false)
  assert.ok(stamped.startsWith('# Plain\n'))
  assert.ok(stampCarve('# Plain\n', 'my-app 1.2', 'block').includes('%%%'))
  assert.ok(!stampCarve('# Plain\n', 'my-app 1.2', 'line').includes('%%%'))
  assert.throws(() => stampCarve('# Plain\n', 'my-app 1.2', 'nope'), TypeError)
  // Stamping twice replaces the marker rather than stacking two.
  assert.equal(stampCarve(stamped, 'my-app 1.3'), stampCarve('# Plain\n', 'my-app 1.3'))
}

// The SVG sanitizer, strict by default.
{
  const hostile = '<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script><rect/></svg>'
  const strict = sanitizeSvg(hostile)
  assert.equal(strict.ok, true)
  assert.ok(!strict.svg.includes('<script'))
  assert.ok(strict.svg.includes('<rect'))
  assert.equal(sanitizeSvg('not svg at all').ok, false)
  // An option is read rather than ignored.
  const linked = '<svg xmlns="http://www.w3.org/2000/svg"><a href="https://example.com/"><rect/></a></svg>'
  assert.ok(!sanitizeSvg(linked).svg.includes('<a '))
  assert.ok(sanitizeSvg(linked, { allowLinks: true }).svg.includes('<a '))
  assert.throws(() => sanitizeSvg(linked, { allowLinks: 'yes' }), TypeError)
}

// Citation locators, the same shape carve-js parses them into.
assert.deepEqual(parseLocator('pp. 12-14'), {
  label: 'page',
  value: '12-14',
  suffixText: null,
})
assert.deepEqual(parseLocator(''), { label: null, value: null, suffixText: null })

// The source-layout sidecar: the byte-exact record of the source, which
// `parseJson`'s semantic positions are not.
{
  const layout = JSON.parse(parseSourceLayoutJson('# One\r\n'))
  assert.equal(layout.version, 1)
  assert.equal(layout.lineEndings, 'crlf')
  assert.equal(layout.bom, false)
  assert.ok(layout.nodes.length > 0)
  assert.equal(typeof layout.nodes[0].path, 'string')
  assert.equal(JSON.parse(parseSourceLayoutJson('# One\n')).lineEndings, 'lf')
}

// The Markdown importer, straight to the tree instead of via Carve source. A
// setext heading is the discriminator: Markdown makes it a heading, and Carve
// has no such form, so a tree that merely re-parsed the input as Carve would
// carry a paragraph here.
{
  const tree = JSON.parse(markdownToAstJson('Title\n=====\n'))
  assert.equal(tree.type, 'document')
  assert.equal(tree.children[0].type, 'heading')
  assert.equal(tree.children[0].level, 1)
  assert.notEqual(JSON.parse(parseJson('Title\n=====\n')).children[0].type, 'heading')
}

// The importers that had no binding. Djot swaps the emphasis delimiters, which
// is exactly why pasting Djot in as Carve renders wrongly rather than failing.
for (const result of [migrateDjot('_em_ and *strong*\n'), migrateBbcode('[b]bold[/b]')]) {
  assert.equal(result.report.schemaVersion, 2)
  assert.deepEqual(
    result.report.diagnostics.map(({ code, fidelity, confidence }) => ({ code, fidelity, confidence })),
    [{ code: 'fidelity-unverified', fidelity: 'dropped', confidence: 'fallback' }],
  )
}
assert.ok(fromDjot('_em_ and *strong*\n').includes('/em/'))
assert.ok(fromBbcode('[b]bold[/b]').includes('*bold*'))
const htmlMigration = htmlToCarve('<p onclick="x()">safe</p>', 'safe')
assert.equal(htmlMigration.report.schemaVersion, 2)
assert.equal(htmlMigration.report.sourceFormat, 'html')
assert.equal(htmlMigration.report.mode, 'safe')
assert.equal(htmlMigration.report.adapter, 'generic')
assert.ok(htmlMigration.report.diagnostics.some(({ code, fidelity, confidence }) =>
  code === 'attribute-dropped' && fidelity === 'dropped' && confidence === 'exact'))
const markdownMigration = fromMarkdown('**bold**')
assert.deepEqual(
  markdownMigration.report.diagnostics.map(({ code, fidelity, confidence }) => ({ code, fidelity, confidence })),
  [{ code: 'fidelity-unverified', fidelity: 'dropped', confidence: 'fallback' }],
)
console.log('wasm artifact: tree, lint, stamp and importer entry points pass')

// The ProseMirror bridge. ProseMirror runs in a browser and nowhere else, so
// this binding is the whole audience for the engine's bridge.
const pm = toProseMirror('A *bold /and italic/* word.')
// A JSON STRING, the same choice parseJson makes - the host parses it natively.
assert.equal(typeof pm.json, 'string')
assert.deepEqual(JSON.parse(pm.json), {
  type: 'doc',
  content: [
    {
      type: 'paragraph',
      content: [
        { type: 'text', text: 'A ' },
        { type: 'text', text: 'bold ', marks: [{ type: 'bold' }] },
        { type: 'text', text: 'and italic', marks: [{ type: 'bold' }, { type: 'italic' }] },
        { type: 'text', text: ' word.' },
      ],
    },
  ],
})
assert.deepEqual(pm.dropped, {})
assert.deepEqual(pm.degraded, {})

// A code block keeps its language as an attribute rather than in the text.
const fenced = JSON.parse(toProseMirror('``` rust\nlet x = 1;\n```\n').json).content[0]
assert.equal(fenced.type, 'codeBlock')
assert.equal(fenced.attrs.language, 'rust')
assert.deepEqual(fenced.content, [{ type: 'text', text: 'let x = 1;' }])

// What the ProseMirror model cannot hold is REPORTED, not silently converted,
// and the two maps say different things. `dropped` is content that is gone;
// `degraded` is content that survives without its node type. A soft break
// becomes whitespace, smart typography resolves to the glyph, so the author's
// `...` does not survive the trip back - and an abbreviation definition has no
// editor node at all.
const softBreak = toProseMirror('left\nright')
assert.equal(softBreak.degraded.soft_break, 'a soft break is whitespace in the ProseMirror model')
assert.deepEqual(softBreak.dropped, {})
assert.equal(
  toProseMirror('a ... b').degraded.smart_punctuation,
  'smart-typography output is lossy on reparse, so it is not modeled',
)
const abbreviated = toProseMirror('*[HTML]: HyperText Markup Language\n\nHTML is fine.\n')
assert.deepEqual(abbreviated.dropped, {
  abbreviation_def: "abbreviation definitions ride on the doc node's attrs",
})
assert.deepEqual(abbreviated.degraded, {})
assert.equal(fromProseMirror(toProseMirror('a ... b').json), 'a … b\n')

// The editor loop closes: source in, source back.
const editable = 'A *bold /and italic/* word.\n\n- one\n- two\n\n> quoted\n'
assert.equal(fromProseMirror(toProseMirror(editable).json), editable)

// A payload the schema map does not describe is refused rather than written
// approximately.
assert.throws(() => fromProseMirror('{"type":"nope"}'), Error)
assert.throws(() => fromProseMirror('{'), Error)
console.log('wasm artifact: ProseMirror bridge passes')




// The PART 12 exchange shape, through the same artifact. A binding that can only
// render is unusable for an editor, a linter or a converter - they need the tree.
const ast = JSON.parse(parseJson('---\ntitle: T\n---\n\nBody[^a].\n\n[^a]: note\n'))

// The root carries exactly three fields (PART 12 §7): frontmatter and footnote
// definitions are block nodes in the tree, not root fields.
assert.deepEqual(Object.keys(ast).sort(), ['children', 'srcByteLength', 'type'])
assert.deepEqual(
  ast.children.map((n) => n.type),
  ['frontmatter', 'paragraph', 'footnote'],
)
// Raw, not a parsed mapping - a parsed map cannot be serialized back to the
// bytes the author wrote.
assert.equal(ast.children[0].content, 'title: T')

// Positions are CODEPOINTS (§4). Bytes and UTF-16 units agree with codepoints
// below U+10000, so the astral character is what makes a wrong unit visible: the
// emoji is 4 bytes, 2 UTF-16 units and 1 codepoint, so only a codepoint index
// puts the strong at column 3.
const astral = JSON.parse(parseJson('\u{1F600} *b*\n'))
const strong = astral.children[0].children.at(-1)
assert.equal(strong.type, 'strong')
assert.equal(strong.pos.startColumn, 3)
assert.equal(strong.pos.startOffset, 2)

console.log('wasm artifact: AST cases pass')

// `{{ path }}` expansion through a JS resolver (markup-carve/carve-wasm#115).
//
// The resolver is SYNCHRONOUS, so the host this serves is one whose files are
// already in memory. Each case is driven on its own: "resolved", "refused by
// the host" and "the binding could not read what came back" are three different
// outcomes and a combined assertion cannot say which fired.
{
  const files = new Map([
    ['child.crv', 'Included body.\n'],
    ['a.crv', 'A\n\n{{ b.crv }}\n'],
    ['b.crv', 'B\n'],
  ])
  const resolve = (path) => files.get(path) ?? null
  const run = (source, extra = {}) => expandIncludes(source, { resolve, ...extra })

  // The child's body reaches the tree, and the target is reported as a
  // dependency a host can watch.
  const basic = run('Before.\n\n{{ child.crv }}\n')
  assert.ok(astJsonToHtml(basic.json).includes('<p>Included body.</p>'), basic.json)
  assert.deepEqual(basic.warnings, [])
  assert.deepEqual(basic.dependencies, [{ id: 'child.crv', resolved: true, denial: null }])
  assert.equal(basic.chargedBytes, 15)
  assert.deepEqual(basic.resolverErrors, [])

  // Transitive expansion, in first-encounter order.
  assert.deepEqual(
    run('{{ a.crv }}\n').dependencies.map((d) => d.id),
    ['a.crv', 'b.crv'],
  )

  // The `ctx` argument, which is what relative resolution keys off. A binding
  // that handed over only the path would pass every assertion above.
  const seen = []
  expandIncludes('{{ a.crv }}\n', {
    sourcePath: 'root.crv',
    resolve: (path, ctx) => {
      seen.push([path, ctx.sourcePath, [...ctx.stack], ctx.depth])
      return resolve(path)
    },
  })
  assert.deepEqual(seen, [
    ['a.crv', 'root.crv', ['root.crv'], 0],
    ['b.crv', 'root.crv', ['root.crv', 'a.crv'], 1],
  ])

  // AN ASYNC RESOLVER, the case the ruling is about. A Promise is not a source
  // and cannot be awaited, so it is reported rather than swallowed - the same
  // treatment `renderers.math` gives one.
  const promised = expandIncludes('{{ child.crv }}\n', { resolve: async (p) => files.get(p) })
  assert.equal(promised.resolverErrors.length, 1)
  assert.equal(promised.resolverErrors[0].path, 'child.crv')
  assert.match(promised.resolverErrors[0].message, /Promise/)
  // The directive stays literal, and the engine's own warning says so too.
  assert.equal(promised.warnings[0].rule, 'include-unresolved')
  assert.ok(!astJsonToHtml(promised.json).includes('Included body'))

  // A THROW is reported with its message, and does not propagate: the engine's
  // resolver returns a Result and cannot unwind.
  const threw = expandIncludes('{{ child.crv }}\n', {
    resolve: () => {
      throw new Error('disk on fire')
    },
  })
  assert.match(threw.resolverErrors[0].message, /disk on fire/)

  // Any other unreadable return is reported too, named by its typeof.
  assert.match(
    expandIncludes('{{ child.crv }}\n', { resolve: () => 42 }).resolverErrors[0].message,
    /number/,
  )

  // A REFUSAL the host meant is not a binding failure: nothing in
  // `resolverErrors`, and the class reaches `dependencies[].denial`.
  const refused = expandIncludes('{{ child.crv }}\n', {
    resolve: () => ({ denial: 'outside-root' }),
  })
  assert.deepEqual(refused.resolverErrors, [])
  assert.equal(refused.dependencies[0].denial, 'outside-root')
  // `null` is the shorthand for the commonest one.
  assert.equal(run('{{ nope.crv }}\n').dependencies[0].denial, 'not-found')
  // An unknown class is a failure, not a silent fallback.
  assert.match(
    expandIncludes('{{ child.crv }}\n', { resolve: () => ({ denial: 'nope' }) })
      .resolverErrors[0].message,
    /unknown denial/,
  )

  // A canonical id is what the cycle guard compares, so it has to survive.
  assert.equal(
    expandIncludes('{{ child.crv }}\n', {
      resolve: () => ({ source: 'X\n', id: '/abs/child.crv' }),
    }).dependencies[0].id,
    '/abs/child.crv',
  )

  // THE BUDGETS. Each is driven on its own, because each produces a different
  // rule and a host lowering one wants to know which it hit.
  assert.equal(run('{{ a.crv }}\n', { maxDepth: 1 }).warnings[0].rule, 'include-depth')
  assert.equal(run('{{ child.crv }}\n', { maxBytes: 1 }).warnings[0].rule, 'include-budget')
  assert.equal(
    run('{{ child.crv }}\n\n{{ child.crv }}\n', { maxResolverCalls: 1 }).warnings[0].rule,
    'include-call-limit',
  )
  // Their DEFAULTS are the contract, so the same documents pass without them.
  assert.deepEqual(run('{{ a.crv }}\n').warnings, [])
  assert.deepEqual(run('{{ child.crv }}\n\n{{ child.crv }}\n').warnings, [])

  // `extensions` reaches the CHILD parse, not only the root. The same text has
  // to mean the same thing in either file, or an include changes what a
  // construct is by being in a different file.
  const wiki = new Map([['w.crv', 'see [[Page]] here\n']])
  const withExt = expandIncludes('{{ w.crv }}\n', {
    resolve: (path) => wiki.get(path) ?? null,
    extensions: ['wikilinks'],
  })
  assert.ok(withExt.json.includes('"wikilink"'), withExt.json)
  const withoutExt = expandIncludes('{{ w.crv }}\n', { resolve: (path) => wiki.get(path) ?? null })
  assert.ok(withoutExt.json.includes('see [[Page]] here'), withoutExt.json)

  // READ-TIME validation. A missing resolver throws rather than expanding
  // nothing: the engine's pass with no resolver is a silent no-op.
  assert.throws(() => expandIncludes('x\n', {}), TypeError)
  assert.throws(() => expandIncludes('x\n', { resolve: 'nope' }), TypeError)
  assert.throws(() => expandIncludes('x\n', { resolve, maxDepth: 'big' }), TypeError)
  assert.throws(() => expandIncludes('x\n', { resolve, maxBytes: -1 }), TypeError)
  assert.throws(() => expandIncludes('x\n', { resolve, extensions: ['no-such'] }), TypeError)
}
console.log('wasm artifact: include expansion cases pass')
// Snapshot parsing and edit-validated re-parse (markup-carve/carve-wasm#111).
//
// The snapshot crosses as JSON, so these are pure functions over strings like
// every other entry point here - there is nothing to `free()`.
{
  const first = JSON.parse(parseSnapshot('# One\n'))
  assert.equal(first.source, '# One\n')
  assert.equal(first.document.children[0].type, 'heading')
  // A first parse covers the whole document.
  assert.deepEqual(first.changedSource, [[0, 6]])
  assert.equal(first.reusedPreviousTree, false)
  // The byte-exact source record rides along, so a host writing an edit back
  // into the file does not need a second call.
  assert.equal(first.sourceLayout.source, '# One\n')

  // One edit, applied and re-parsed. The tree has to CHANGE, or the binding
  // would be handing back the parse it was given.
  const next = JSON.parse(reparse(first.source, '[{"range":[2,5],"replacement":"Two"}]'))
  assert.equal(next.source, '# Two\n')
  assert.equal(next.document.children[0].attrs.id, 'Two')
  // The changed ranges are the edits, not the whole document.
  assert.deepEqual(next.changedSource, [[2, 5]])

  // Two edits are both applied, and reported in source order. Applying them
  // left to right without accounting for the shift would corrupt the second.
  const two = JSON.parse(
    reparse('abcdef\n', '[{"range":[0,1],"replacement":"X"},{"range":[4,5],"replacement":"Y"}]'),
  )
  assert.equal(two.source, 'XbcdYf\n')
  assert.deepEqual(two.changedSource, [[0, 1], [4, 5]])

  // THE OFFSETS ARE BYTES, which is the whole reason this needs saying: `é` is
  // two of them, and a host counting UTF-16 code units would pass 1.
  assert.equal(JSON.parse(reparse('é b\n', '[{"range":[0,2],"replacement":"e"}]')).source, 'e b\n')
  assert.throws(() => reparse('é b\n', '[{"range":[0,1],"replacement":"e"}]'), /code point/)

  // A BROKEN CALLER CONTRACT throws, driven one at a time because each names a
  // different mistake and a host wants to know which it made.
  assert.throws(
    () => reparse('abcdef\n', '[{"range":[0,3],"replacement":"X"},{"range":[2,4],"replacement":"Y"}]'),
    /overlap/,
  )
  assert.throws(() => reparse('abc\n', '[{"range":[0,99],"replacement":"X"}]'), /out of bounds/)
  assert.throws(() => reparse('abc\n', 'nope'), /not JSON/)
  assert.throws(() => reparse('abc\n', '{}'), /must be a JSON array/)
  assert.throws(() => reparse('abc\n', '[{"replacement":"X"}]'), /needs a `range`/)
  assert.throws(() => reparse('abc\n', '[{"range":[0,1]}]'), /needs a `replacement`/)
  assert.throws(() => reparse('abc\n', '[{"range":[-1,1],"replacement":"X"}]'), /non-negative/)

  // An empty change list re-parses the source unchanged, rather than being an
  // error - a host batching keystrokes can hit it.
  const idle = JSON.parse(reparse('# One\n', '[]'))
  assert.equal(idle.source, '# One\n')
  assert.deepEqual(idle.changedSource, [])

  // `reusedPreviousTree` is the ENGINE's answer, and today it is always false:
  // the pinned engine validates and applies the edits, then parses the whole
  // source. Pinned so that an engine bump which starts reusing regions shows up
  // here rather than passing unnoticed.
  assert.equal(next.reusedPreviousTree, false)
}
console.log('wasm artifact: incremental parse cases pass')
// The AST patch family (markup-carve/carve-wasm#112).
//
// Trees and patches both cross as JSON strings, so these are pure functions
// over strings. The engine's `ast_patch_to_json` / `ast_patch_from_json` are
// not bound: with the patch crossing as JSON they ARE this pair's encoding.
{
  const one = parseJson('# One\n\nBody.\n')
  const two = parseJson('# Two\n\nBody.\n')

  // The patch is the position-independent wire shape, not a source diff.
  const patch = JSON.parse(createAstPatch(one, two))
  assert.deepEqual(
    patch.map((op) => op.op),
    ['replace', 'replace'],
  )
  assert.ok(patch.every((op) => typeof op.path === 'string' && op.path.startsWith('/')), patch)

  // Replaying it produces the OTHER tree, checked through the source writer so
  // a patch that changed nothing could not pass.
  assert.equal(astJsonToCarve(applyAstPatch(one, createAstPatch(one, two))), '# Two\n\nBody.\n')
  // And the reverse direction, so the arguments cannot be transposed.
  assert.equal(astJsonToCarve(applyAstPatch(two, createAstPatch(two, one))), '# One\n\nBody.\n')

  // No difference is an empty patch rather than an error.
  assert.deepEqual(JSON.parse(createAstPatch(one, one)), [])
  assert.equal(astJsonToCarve(applyAstPatch(one, '[]')), '# One\n\nBody.\n')

  // THE REVERSIBLE PAIR, which is what an undo step wants. The stack stays the
  // host's; this package holds no history.
  const reversible = JSON.parse(createReversibleAstPatch(one, two))
  assert.deepEqual(Object.keys(reversible).sort(), [
    'afterFingerprint',
    'beforeFingerprint',
    'forward',
    'inverse',
  ])
  assert.match(reversible.beforeFingerprint, /^fnv1a64:/)
  assert.notEqual(reversible.beforeFingerprint, reversible.afterFingerprint)
  // Forward and inverse are genuinely different lists, not one list twice.
  assert.notDeepEqual(reversible.forward, reversible.inverse)

  const wire = JSON.stringify(reversible)
  assert.equal(astJsonToCarve(applyReversibleAstPatch(one, wire)), '# Two\n\nBody.\n')
  assert.equal(astJsonToCarve(applyReversibleAstPatch(two, wire, true)), '# One\n\nBody.\n')

  // THE PRECONDITION is what the pair adds over two `createAstPatch` calls: an
  // undo cannot be replayed onto a document that has moved on.
  assert.throws(() => applyReversibleAstPatch(two, wire), /precondition/)
  assert.throws(() => applyReversibleAstPatch(one, wire, true), /precondition/)

  // Bad input is named by WHICH argument was bad, so a host is not left
  // guessing which of two trees it mis-serialized.
  assert.throws(() => createAstPatch('nope', two), /`before`/)
  assert.throws(() => createAstPatch(one, 'nope'), /`after`/)
  assert.throws(() => applyAstPatch('nope', '[]'), /`ast`/)
  assert.throws(() => applyAstPatch(one, 'nope'), Error)
  assert.throws(() => applyAstPatch(one, '{}'), /must be an array/)
  assert.throws(() => applyReversibleAstPatch(one, 'nope'), /not JSON/)
  assert.throws(
    () => applyReversibleAstPatch(one, '{"inverse":[],"beforeFingerprint":"a","afterFingerprint":"b"}'),
    /`forward`/,
  )
  assert.throws(() => applyReversibleAstPatch(one, '{"forward":[],"inverse":[]}'), /Fingerprint/)
}
console.log('wasm artifact: AST patch family cases pass')
