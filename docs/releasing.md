# Releasing carve-wasm

Publishing is gated. `.github/workflows/release.yml` has two jobs, and
`publish` declares `verify` in `needs:`, so it cannot start while the gate
fails. The gate is `scripts/verify-release-artifact.mjs`, and it is runnable by
hand against a local build:

```sh
npm ci --ignore-scripts
npx playwright install --with-deps chromium firefox webkit
npm run build
CARVE_SPEC_CORPUS=/path/to/carve/tests/corpus node scripts/verify-release-artifact.mjs
```

It runs `npm pack` and drives the smoke, corpus, HTML roundtrip and ProseMirror
suites at the unpacked tarball. It also installs the package and checks Node,
TypeScript, Vite and all three browser engines. The difference matters: npm uploads what the
generated `files` list names, so a payload file left out of it would never
reach the registry and would never have been tested either. The corpus
population comes from the spec's example pages, so a truncated corpus fails
here instead of passing over a subset, and an unset `CARVE_SPEC_CORPUS` is
refused rather than skipped.

## What this release waits on

carve-wasm has one cross-repository edge that is not in any manifest, and it
decides release ORDER rather than content. `.github/workflows/ci.yml` checks out
`markup-carve/carve-rb` and runs `scripts/check-engine-floor.py` against that
repository's engine pin, which must not be ahead of this one. The org's
`tools/dependency-map.mjs` does see the edge, as `kind: ci` from this workflow
path, so it is not invisible to tooling; what no file states is the consequence.

- **Do not tag carve-wasm ahead of the carve-rb release its floor is read
  from.** Shipping a wasm package whose declared floor sibling has not released
  the engine both of them pin leaves two bindings of one engine that a host
  cannot reason about together.
- **The floor is read from carve-rb's DEFAULT BRANCH, not from a carve-rb
  release.** A sibling's in-flight bump can therefore turn this repository's CI
  red with no commit here and nothing to fix here. That has not bitten yet;
  whether the floor should read the sibling's latest tag instead is open in
  markup-carve/carve-wasm#160.
- carve-wasm was missing from the release fleet list, which is a separate file
  from the dependency map, and that is why a wave skipped it and a pin sat 34
  commits behind. Adding a repository to this org means adding it there too.

## Cutting one

1. Merge the version bump: `Cargo.toml` and the `CHANGELOG.md` section have to
   name the version being released. The workflow refuses a tag whose version
   disagrees with `Cargo.toml`.
2. Write the notes as an unpublished draft release for the intended version and
   exact commit:
   `gh release create vX.Y.Z --draft --target COMMIT --notes-file NOTES.md`. The workflow will
   not publish without one. Copy the tagged version's `CHANGELOG.md` section or
   summarize it,
   excluding its version heading. Resolve relative links to that tag, then add
   `**Full Changelog**: https://github.com/markup-carve/carve-wasm/compare/vPREVIOUS...vX.Y.Z`.
   Include a `[Changelog](https://github.com/markup-carve/carve-wasm/blob/vX.Y.Z/CHANGELOG.md)`
   link for the intended tag. A shorter summary is allowed if it covers the
   breaking changes, cites each breaking entry's ticket when present, and
   references only tickets from that section. Include at least one ticket
   reference if the section has any. Rehearse the notes
   gate with `gh workflow run rehearse-release-notes.yml -f tag=vX.Y.Z` before
   tagging. After every draft mutation, read it through the API and verify
   `tag_name`, `target_commitish` and `draft: true`. Do not push a tag or publish
   the package while preparing the draft.
3. Push the tag. The gate builds and verifies the packed tarball, npm publishes,
   and the last step flips the draft to published.

Two things that have gone wrong here before:

- **A tag pushed before the version bump merges** fails the version-agreement
  step, and nothing publishes. Merge first.
- **Deleting and re-pushing a tag silently converts its PUBLISHED release back
  into a DRAFT.** Exit status will not tell you. After any tag surgery, read the
  release back:
  `gh api repos/markup-carve/carve-wasm/releases --jq '.[] | "\(.draft) \(.tag_name)"'`
  and confirm the tag is the version rather than an `untagged-*` placeholder.
