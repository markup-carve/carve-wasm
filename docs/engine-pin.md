# Rust 0.2 engine pin

`Cargo.toml` pins the unreleased `carve2` crate to a full commit on carve-rs `next2`. The crate remains unpublished. `Cargo.lock` records the same Git revision. A pin change updates both files and runs `cargo test --locked`, the WASM build, and the Node smoke tests.

The Rust engine's `v2/README.md` is the migration guide and API reference. Its `api::json::call` function is the binding boundary. The writer, HTML importer and extension host gaps are tracked in carve-rs issues 4327, 4328 and 4329.
