# Changelog

Notable changes to `@markup-carve/carve-wasm`.

The parser and renderer are carve-rs, compiled to WebAssembly and pinned to an
exact commit in `Cargo.toml`, so an engine bump can change rendering without a
line of Rust in this repository changing. Engine bumps therefore get an entry of
their own.

## [Unreleased]

## [0.1.0] - 2026-08-18

First release. Nothing has been published to npm before this, so there is no
version a reader can be upgrading from.

- WebAssembly bindings for the Carve parser and HTML renderer: `toHtml`, the
  extension surface, the `sections` option, the static render mode, and the
  parsed AST.
- Embeds carve-rs at `9705274c` (crate version `0.1.3`, 33 commits past the
  `0.1.3` tag), which includes the PART 9 §25 fix where a list-valued URL
  attribute is probed at every candidate rather than at its head -
  `srcset="safe.png 1x, javascript:alert(1) 2x"` used to pass on its second
  entry.
- The publish is gated: `.github/workflows/release.yml` runs
  `scripts/verify-release-artifact.mjs` against the packed npm tarball and the
  publish job declares that gate in `needs:`, so a tarball that renders the spec
  corpus differently cannot reach the registry.

[Unreleased]: https://github.com/markup-carve/carve-wasm/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/markup-carve/carve-wasm/releases/tag/v0.1.0
