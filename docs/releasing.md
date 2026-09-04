# Releasing carve-wasm

Publishing is gated. `.github/workflows/release.yml` has two jobs, and
`publish` declares `verify` in `needs:`, so it cannot start while the gate
fails. The gate is `scripts/verify-release-artifact.mjs`, and it is runnable by
hand against a local build:

```sh
wasm-pack build --target bundler --scope markup-carve
CARVE_SPEC_CORPUS=/path/to/carve/tests/corpus node scripts/verify-release-artifact.mjs
```

It runs `npm pack` and drives `tests/smoke.mjs` and `tests/corpus.mjs` at the
UNPACKED tarball, not at `pkg/`. The difference matters: npm uploads what the
generated `files` list names, so a payload file left out of it would never
reach the registry and would never have been tested either. The corpus
population comes from the spec's example pages, so a truncated corpus fails
here instead of passing over a subset, and an unset `CARVE_SPEC_CORPUS` is
refused rather than skipped.

## Cutting one

1. Merge the version bump: `Cargo.toml` and the `CHANGELOG.md` section have to
   name the version being released. The workflow refuses a tag whose version
   disagrees with `Cargo.toml`.
2. Write the notes as a DRAFT release for the tag:
   `gh release create vX.Y.Z --draft --notes-file NOTES.md`. The workflow will
   not publish without one - a CHANGELOG entry is written for a contributor and
   makes poor reading for someone deciding whether to upgrade.
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
