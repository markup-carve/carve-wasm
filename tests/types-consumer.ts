import {
  expandIncludes, lintAccessibility, lintCarve, lintCarveWithOptions, toHtmlWithReport, toHtmlWithOptions, toHtmlWithRenderers,
  createSourcePatch, applySourcePatch, ParserSession, selectIncludeRegion, toCarveWithOptions,
  type AccessibilityDiagnostic, type IncludeExpansion, type IncludeLintWarning, type IncludeRegion, type LintOptions, type LintWarning, type RenderResult,
} from '@markup-carve/carve-wasm'
const expanded: IncludeExpansion = expandIncludes('{{ child }}', {
  resolve: (path, context) => ({ source: context.stack.join('/') + path, id: path }),
  maxBytes: 1000,
})
const report: RenderResult = toHtmlWithReport('body')
report.losses.map(loss => loss.pos?.startOffset)
// The per-code totals a truncated report has to be read through.
const perCode: number = report.totalsByCode['raw-format-dropped']
// The lint family's own pair, in UTF-16 code units, which is what indexes the
// string a JS caller holds.
const warnings: LintWarning[] = lintCarve('body')
warnings.map(warning => 'x'.slice(warning.start, warning.end))
warnings.map(warning => warning.includedBy.map(site => site.file + warning.file))
const chained: IncludeLintWarning[] = lintCarveWithOptions('body', { includes: { resolve: () => null, maxDepth: 4 } })
chained.map(warning => warning.start === null ? -1 : warning.start)
// @ts-expect-error Offsets of a lint that follows includes may be null.
const notNull: number = lintCarveWithOptions('body', { includes: { resolve: () => null } })[0].start
void notNull
const lintOptions: LintOptions = { includes: { resolve: () => null } }
// @ts-expect-error A LintOptions value may carry includes, so its offsets may be null.
const maybeNull: number = lintCarveWithOptions('body', lintOptions)[0].start
void maybeNull
const plain: LintWarning[] = lintCarveWithOptions('body', { extensions: [] })
void plain
const a11y: AccessibilityDiagnostic[] = lintAccessibility('body')
a11y.map(item => item.startUtf16 === null ? -1 : item.startUtf16)
void perCode
expanded.dependencies.map(dependency => dependency.denial)
toHtmlWithOptions('body', { rawHtml: false, profile: 'comment', asciiHeadingIds: 'fold' })
toCarveWithOptions('|a|b|', { alignTables: true })
const region: IncludeRegion = selectIncludeRegion('%% #region a\nx\n%% #endregion\n', 'a', 'code')
void region.end
expandIncludes('{{ bin }}', { resolve: () => ({ denial: 'include-non-text' }) })
// @ts-expect-error A region mode is "carve" or "code".
selectIncludeRegion('body', 'a', 'markdown')
toHtmlWithRenderers('body', { renderers: { diagrams: { mermaid: source => source }, math: (tex, display) => display ? tex : '' } })
applySourcePatch('a', createSourcePatch('a', 'b', 'refactor', 'change'))
// @ts-expect-error Report strictness is a boolean.
toHtmlWithReport('body', 'false')
// @ts-expect-error Renderer options reject misspelled fields.
toHtmlWithOptions('body', { rawHTML: false })
// @ts-expect-error An include resolver is synchronous.
expandIncludes('body', { resolve: async () => 'body' })
// @ts-expect-error Source patches require a version and fingerprint.
applySourcePatch('body', {})

const session = new ParserSession('body')
session.snapshot()
session.edit('[]')
session.free()

import { toHtmlWithOptions as renderHtml } from '@markup-carve/carve-wasm/render'
import initWeb, { ParserSession as WebSession } from '@markup-carve/carve-wasm/web'
import { utf16ToUtf8 } from '@markup-carve/carve-wasm/offsets'
renderHtml('body')
void initWeb
void WebSession
utf16ToUtf8('😀', 2)
// @ts-expect-error The rendering-only build has no AST parser.
import { parseJsonWithOptions } from '@markup-carve/carve-wasm/render'
// @ts-expect-error The rendering-only build has no checked reports.
import { toHtmlWithReport as absentReport } from '@markup-carve/carve-wasm/render'
// @ts-expect-error The rendering-only build has no Markdown output.
import { toMarkdownWithOptions } from '@markup-carve/carve-wasm/render'
