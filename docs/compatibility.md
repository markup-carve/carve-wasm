# Development package compatibility

The package builder emits a bundler entry at the root, a synchronous Node entry in `node/`, a browser entry in `web/`, and a `render/` browser entry. All four use the same pinned 0.2 engine. Browser entries need `await init()`; the Node entry initializes automatically.

The 0.2 binding on `main` has no published package yet. Build it with `npm run build` and import from `pkg/`. The 0.1 package's tree, importer, editor and writer results are not compatible with the 0.2 schema. Use `call` for the complete engine boundary or the convenience functions in [the reference](reference.md).

The example worker receives `render` and `parse` requests and replies with either `value` or `error`. Its methods are synchronous after the web module has initialized.

The `offsets` module still converts UTF-8 byte, UTF-16 code unit, and codepoint offsets for hosts that need them.
