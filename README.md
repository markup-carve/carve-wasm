# carve-wasm

WASM bindings for [carve-rs](https://github.com/markup-carve/carve-rs), the
reference Rust parser. They let the Rust implementation run client-side in the
browser and back the **Rust (WASM)** engine in the
[Carve playground](https://markup-carve.github.io/carve/playground).

## API

- `toHtml(source: string): string`
- `version(): string`

## Build

```bash
cargo test
wasm-pack build --target web --scope markup-carve
```

`wasm-pack` emits the browser package into `pkg/`.

## Example

```js
import init, { toHtml } from './pkg/carve_wasm.js'

await init()
document.body.innerHTML = toHtml('# Hello')
```
