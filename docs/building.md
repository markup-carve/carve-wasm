# Building the Carve 0.2 WASM package

Use Rust 1.88 or later with the `wasm32-unknown-unknown` target, Node 22 or later, and wasm-pack 0.15.0. The engine is fetched from its pinned Git commit. No registry release is needed.

```sh
export CARGO_BUILD_JOBS=4
cargo test --locked
npm ci --ignore-scripts
npm run build
npm test
```

The build emits bundler, Node and browser packages under `pkg/`. `npm test` exercises the built Node artifact. The 0.2 CI workflow checks Rust formatting, native tests, the WASM target and that artifact. The previous 0.1 corpus, editor and importer tests stay in the branch history; their APIs are not part of this 0.2 package.

For the pinned 0.2 render examples, set `CARVE_V2_EXAMPLES` to `spec-conformance/examples.json` from the exact engine commit, then run `npm run test:conformance`. The binding passes all 505 render examples.
