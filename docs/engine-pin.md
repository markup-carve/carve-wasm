# The carve-rs dependency pin

`Cargo.toml` pins an exact carve-rs commit, and `Cargo.lock` is committed
alongside it:

```toml
carve = { package = "carve-lang", git = "https://github.com/markup-carve/carve-rs", rev = "..." }
```

Read the current revision out of `Cargo.toml` rather than from a copy here - a
revision quoted in prose goes stale the first time someone bumps the manifest
without noticing the duplicate.

The engine is published as `carve-lang` (carve-rs renamed it from `carve`), so a
pin at any revision past that rename needs `package = "carve-lang"` as above.

The crate previously tracked carve-rs' default branch with no committed lock.
That never went stale, but it went the other way: every build resolved whatever
had landed upstream since, so the published package could carry an engine no CI
run here had ever built, and two clones a day apart could disagree. The pin
makes an engine change a reviewable line in a diff.

When bumping the `rev`, regenerate and commit `Cargo.lock` in the same change.
The lock records the resolved revision plus the rest of the tree; leaving it
behind gives every fresh clone a dirty working tree on its first build and lets
the package resolve to an engine other than the one that was tested.

```sh
cargo update -p carve-lang --precise <sha>   # or edit the rev and re-lock
cargo test && wasm-pack build --target nodejs && node tests/smoke.mjs
CARVE_SPEC_CORPUS=/path/to/carve/tests/corpus node tests/corpus.mjs
```

`scripts/check-engine-floor.py` is what notices a pin left behind. CI runs it
against the revision carve-rb embeds:

```sh
python3 scripts/check-engine-floor.py \
    --engine <carve-rs checkout> --manifest Cargo.toml --lock Cargo.lock \
    --sibling-name carve-rb --sibling-manifest <carve-rb>/ext/carve/Cargo.toml \
    --changelog CHANGELOG.md
```

It fails when this pin is a strict ancestor of the sibling's, and it also fails
when the `[Unreleased]` changelog section names a revision the build does not
embed - a revision quoted in prose is a second copy of the pin, and the second
copy is the one nothing else reads. A floor rather than a leash: it is
deliberately not a distance check against carve-rs `main`, which merges
continuously and would be red from the moment any pull request opens there.

That last line is the one that can tell a drifted pin from a current one.
`smoke.mjs` asserts hand-written expectations, which a stale engine satisfies
happily; `corpus.mjs` renders all ~530 mandatory spec documents through the
**built** artifact and requires byte-identical HTML. Without `CARVE_SPEC_CORPUS`
it prints a notice and exits 0, so a checkout without the spec repo still runs
the suite. CI always sets it.

It measures the binding as much as the engine: carve-rs is corpus-checked
upstream, but that says nothing about whether the wasm-bindgen layer drops a
field or mangles an option on the way through.

Regenerate the whole lock MSRV-aware, or it will quietly break the `rust-version`
this crate advertises. A plain `cargo generate-lockfile` on a current toolchain
picks the newest `wasm-bindgen`, which needs a newer Rust than the 1.75 declared
above - fine while CI only runs `stable`, and a hard failure for anyone actually
building on the floor:

```sh
CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo generate-lockfile
```

Nothing in CI catches this today, since the workflow uses `stable` only. Adding
a 1.75 job (carve-rs has one) would turn it from a review question into a build
failure.
