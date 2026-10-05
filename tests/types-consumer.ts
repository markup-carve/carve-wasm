import {
  expandIncludes, toHtmlWithReport, toHtmlWithOptions, toHtmlWithRenderers,
  createSourcePatch, applySourcePatch, ParserSession, type IncludeExpansion, type RenderResult,
} from '@markup-carve/carve-wasm'
const expanded: IncludeExpansion = expandIncludes('{{ child }}', {
  resolve: (path, context) => ({ source: context.stack.join('/') + path, id: path }),
  maxBytes: 1000,
})
const report: RenderResult = toHtmlWithReport('body')
report.losses.map(loss => loss.pos?.startOffset)
expanded.dependencies.map(dependency => dependency.denial)
toHtmlWithOptions('body', { rawHtml: false, profile: 'comment', asciiHeadingIds: 'fold' })
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
