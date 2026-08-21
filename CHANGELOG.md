# Changelog

Notable changes to `@markup-carve/carve-wasm`.

The parser and renderer are carve-rs, compiled to WebAssembly and pinned to an
exact commit in `Cargo.toml`, so an engine bump can change rendering without a
line of Rust in this repository changing. Engine bumps therefore get an entry of
their own.

## [Unreleased]

### Changed

- Embeds carve-rs at `85514c6b` instead of `9705274c`, 44 commits later and past the revision carve-rb embeds, so the two bindings of this engine no longer render the same document differently (#51). What a reader sees change: rendered elements say what they are called (PART 9 §16a) - the footnote section, footnote backlinks, task-list checkboxes, math spans, admonitions and diagram fences all gain an accessible name; `<thead>` and `<tfoot>` write one row per line like `<tbody>` always did; a table cell's marker run ends at a space; and the doubled run is the canonical arrow. Measured through the built artifact: 1341/1341 mandatory corpus documents byte-identical, up from 227 diverging.

### Added

- `scripts/check-engine-floor.py`, run by CI, fails when the engine pin falls behind the revision carve-rb embeds. The age check passes on a pin bumped inside its window however far behind a sibling it is, and the corpus gate is aimed at the spec commit the pinned engine pins, so an old pin and an old spec stay green together - which is how this pin got 28 commits behind (#51).

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
