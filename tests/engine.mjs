// Resolve the wasm artifact the tests run against.
//
// `tests/smoke.mjs` and `tests/corpus.mjs` measure the BUILT package rather
// than the Rust source, which is the whole reason they exist: `cargo test`
// exercises the native build of the wrapper and cannot tell a drifted engine
// pin, or a broken wasm-bindgen boundary, from a working one.
//
// Which built package, though, was hardcoded to `../pkg`. That is the one
// wasm-pack writes, and it is NOT necessarily the one `npm publish` uploads:
// `files` in the generated package.json decides that, and a file left out of
// it is a file no test here has ever loaded. So the path is a parameter.
//
//   unset                 -> ../pkg, the build CI makes. Unchanged behavior.
//   CARVE_WASM_PKG=<dir>  -> that directory. The release gate points it at the
//                            UNPACKED `npm pack` tarball, so the corpus runs
//                            through the exact bytes about to be published.
//
// Both wasm-pack targets are accepted, because they are not the same artifact:
// CI builds `--target nodejs` and the release publishes `--target bundler`.
// A gate that could only load the nodejs build would be measuring a package
// nobody installs.
import { existsSync, readFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const PKG = process.env.CARVE_WASM_PKG
  ? resolve(process.env.CARVE_WASM_PKG)
  : fileURLToPath(new URL('../pkg/', import.meta.url))

const entry = join(PKG, 'carve_wasm.js')
if (!existsSync(entry)) {
  throw new Error(
    `no wasm package at ${PKG}: ${entry} is missing. Build one with ` +
      '`wasm-pack build --target nodejs`, or point CARVE_WASM_PKG at a built package.',
  )
}

// The bundler target splits the glue out of the entry point; the nodejs target
// does not. That split is the discriminator, and it is structural rather than a
// flag someone has to remember to pass.
const glueFile = join(PKG, 'carve_wasm_bg.js')

const load = async () => {
  if (!existsSync(glueFile)) {
    // nodejs target: the entry point loads its own wasm on require.
    return import(pathToFileURL(entry))
  }

  // bundler target: the entry point is ESM that imports `./carve_wasm_bg.wasm`
  // as a module, which only a bundler (or Node's experimental wasm-modules
  // flag) can resolve. Link it the way a bundler does instead, so the gate
  // needs no flag and no bundler: instantiate the REAL wasm bytes against the
  // REAL glue, then hand the instance back through the hook wasm-bindgen emits
  // for exactly this.
  //
  // `new WebAssembly.Module(bytes)` is deliberate rather than incidental. It is
  // what makes a corrupted or truncated payload a CompileError here instead of
  // a silent fallback, and it is the arm the release gate's ablation exercises.
  const glue = await import(pathToFileURL(glueFile))
  const bytes = readFileSync(join(PKG, 'carve_wasm_bg.wasm'))
  const instance = new WebAssembly.Instance(new WebAssembly.Module(bytes), {
    './carve_wasm_bg.js': glue,
  })
  glue.__wbg_set_wasm(instance.exports)
  instance.exports.__wbindgen_start()
  return glue
}

const engine = await load()

// Named rather than re-exported wholesale. A package that dropped an export, or
// a `files` list that dropped the glue carrying it, fails here with the name in
// the message instead of as `undefined is not a function` three files away.
const NAMES = [
  'extensions',
  'htmlToCarve',
  'parseJson',
  'toHtml',
  'toHtmlFull',
  'toHtmlWithOptions',
  'toHtmlWithSymbols',
  'toHtmlWithReport',
  'toMarkdownWithReport',
  'toPlainTextWithReport',
  'toAnsiWithReport',
  'toCarveWithReport',
  'version',
]
const missing = NAMES.filter((name) => typeof engine[name] !== 'function')
if (missing.length > 0) {
  throw new Error(`the wasm package at ${PKG} does not export: ${missing.join(', ')}`)
}

export const {
  extensions,
  htmlToCarve,
  parseJson,
  toHtml,
  toHtmlFull,
  toHtmlWithOptions,
  toHtmlWithSymbols,
  toHtmlWithReport,
  toMarkdownWithReport,
  toPlainTextWithReport,
  toAnsiWithReport,
  toCarveWithReport,
  version,
} = engine

export const packageUnderTest = PKG
