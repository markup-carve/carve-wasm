# Changelog

Notable changes to `@markup-carve/carve-wasm`.

The parser and renderer are carve-rs, compiled to WebAssembly and pinned to an
exact commit in `Cargo.toml`, so an engine bump can change rendering without a
line of Rust in this repository changing. Engine bumps therefore get an entry of
their own.

## [Unreleased]

### Changed

- Advances the embedded carve-rs revision to `da45f9d2`, matching the current
  carve-rb floor. Djot migration now preserves table continuations
  (carve-rs#1478), empty external-link targets are omitted (carve-rs#1479),
  titled media emits one title attribute (carve-rs#1481), authored task states
  survive format cycles and extended states name themselves in HTML
  (carve-rs#1485, carve-rs#1486), and a colon followed by a space and a tab no
  longer opens a description (carve-rs#1488).

## [0.1.1] - 2026-08-27

### Added

- Checked `to*WithReport` exports for every render target (markup-carve/carve#1728), with bounded positioned losses and strict refusal.
- Shared `fromHtml` and `fromMarkdown` migration entry points with reports.
- `toMarkdown`, `toPlainText`, `toAnsi`, and `toCarve`, completing the core
  carve-rs render-target surface exposed by the WASM package.

### Changed

- Advances the embedded carve-rs revision from `9cf16d05` to released 0.1.4
  (`2e9c43f2`), matching the current sibling binding floor. Documents that used to come
  back rendering differently through the HTML importer now survive it: a task
  item comes back a task item rather than as the checkbox HTML that rendered it
  (carve-rs#1366, carve-rs#1364, carve-rs#1374), and a grouping label keeps its
  div fence (carve-rs#1322). Attached list-marker attributes are now
  layout-transparent, so item bodies use the bare marker's content column
  (carve#1701). Measured through the built artifact: 1394/1394
  corpus documents byte-identical at the spec commit this engine pins.
- Embeds carve-rs at `9cf16d05` instead of `9705274c`, 105 commits later and 32 past the revision carve-rb embeds, so the two bindings of this engine no longer render the same document differently (#51, #56). On top of what `85514c6b` already carried - rendered elements saying what they are called (PART 9 §16a), `<thead>` and `<tfoot>` writing one row per line, a table cell's marker run ending at a space - this run is mostly the HTML importer: a deletion, a math span and a div's content survive an import, an import rebuilds the container the renderer wrote and takes the labels map the HTML was rendered with, and a container's span ends where its markup does. Measured through the built artifact: 1371/1371 mandatory corpus documents byte-identical at the spec commit this engine pins.

### Added

- `toMarkdown`, `toPlainText`, `toAnsi` and `toCarve`, so every core render
  target the embedded engine already understood is reachable from JavaScript
  (#54). `toHtml` and the AST entry points are unchanged, and no rebuild and no
  engine bump is involved: the flags were inside the shipped artifact and had no
  binding.
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

[Unreleased]: https://github.com/markup-carve/carve-wasm/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/markup-carve/carve-wasm/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/markup-carve/carve-wasm/releases/tag/v0.1.0
