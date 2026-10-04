#!/usr/bin/env bash
# The pre-publish release-notes gate, shared by release.yml and rehearse-release-notes.yml
# so the rehearsal runs exactly what the release runs.
#
# Usage: tools/run-release-notes-gate.sh TAG [RELEASE_JSON_OUT]
# Needs GH_TOKEN with contents: write (a draft is invisible to read access) and
# GITHUB_REPOSITORY. Writes the matched release to RELEASE_JSON_OUT when given.
set -eo pipefail

tag="${1:?usage: run-release-notes-gate.sh TAG [RELEASE_JSON_OUT]}"

# Found by listing rather than `gh release view <tag>`: a draft's tag is not a
# git ref, so addressing one by tag can match nothing, change nothing and still
# exit 0.
release="$(gh api "repos/$GITHUB_REPOSITORY/releases?per_page=100" --paginate \
  | jq -cs --arg tag "$tag" '[.[][] | select(.tag_name == $tag)] | first // empty')"
if [ -z "$release" ]; then
  echo "::error::No release for $tag. Write its notes first."
  exit 1
fi
printf '%s' "$release" | python tools/check-release-notes.py \
  --tag "$tag" --repo "$GITHUB_REPOSITORY"

if [ -n "${2:-}" ]; then
  printf '%s' "$release" > "$2"
fi
