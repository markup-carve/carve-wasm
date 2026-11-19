# Rust 0.2 engine pin

`Cargo.toml` pins the unreleased `carve-lang` crate (library `carve`) to a full commit on carve-rs `next2`, with default features off so the WASM build carries no file-system code. The crate remains unpublished. `Cargo.lock` records the same Git revision. A pin change updates both files and runs `cargo test --locked`, the WASM build, and the Node smoke tests.

The Rust engine's `README.md` is the migration guide and API reference. Its `api::json::call` function is the binding boundary. The Markdown, plain-text and ANSI writers are tracked in carve-rs issues 4327 and 4349. Layout sidecars and migration of saved 0.1 trees are tracked in issues 4334 and 4335.
