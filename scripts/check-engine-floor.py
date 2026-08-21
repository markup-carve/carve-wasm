#!/usr/bin/env python3
"""Fail when this package's engine pin is BEHIND the one a sibling binding embeds.

WHY THIS EXISTS. The pin here sat 28 commits behind carve-rb's for two days
with every check green (markup-carve/carve-wasm#51). Nothing was broken in a way
anything measured: `cargo test` and `smoke.mjs` assert hand-written expectations,
which a stale engine satisfies happily; the corpus gate is aimed at the spec
commit THE PINNED ENGINE PINS, so a pin and an old spec move together and the
gate stays green on both; and the `engine-pin` job gates on AGE, so a pin bumped
within the window passes however far behind a sibling it is. Two bindings of one
engine rendered the same document differently and no job could say so.

WHY IT IS A SIBLING FLOOR AND NOT A DISTANCE CHECK AGAINST MAIN. carve-rs merges
continuously, so "fail when behind main" is red from the moment any pull request
opens there and unclearable by the action it recommends - the reasoning in
carve-rs' own tools/check-engine-pin.py, which is why that one measures age. A
sibling's pin is different in the way that matters: it is a revision that a
binding of this same engine has already shipped against, it moves only when
someone deliberately bumps it, and being behind it is cleared by the action this
recommends. It is a floor, not a leash.

WHAT IT ASSERTS, AND WHAT MAKES IT FAIL AT ZERO DRIFT. The comparison is
ancestry, not dates or counts:

  behind      our revision is a strict ancestor of the sibling's -> FAILURE,
              with the commit count, because that is exactly the divergence.
  equal/ahead the sibling's revision is an ancestor of ours -> pass, with the
              distance reported.
  unrelated   neither is an ancestor of the other -> FAILURE. One of the two is
              off the engine's history, which is a worse finding than lag.

Every way of NOT MEASURING is a failure too - an unreadable manifest, a
revision missing from the engine checkout, a sibling manifest that names no
engine. A checker that shrugs and exits 0 is the check that cannot fail
(markup-carve/carve#755), which is the shape that produced the lag it is here
to catch.

    python3 scripts/check-engine-floor.py \\
        --engine carve-rs --manifest Cargo.toml --lock Cargo.lock \\
        --sibling-name carve-rb --sibling-manifest carve-rb/ext/carve/Cargo.toml

Exit codes: 0 at or ahead of the floor, 1 behind it or unable to measure,
2 usage or setup error.
"""

from __future__ import annotations

import argparse
import importlib.util
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent


def _load_pin_reader():
    """Borrow the manifest and lock readers rather than re-spelling them.

    pinned-spec-commit.py already knows that the dependency key differs per
    binding, that the crate publishes as `carve-lang`, and that the lock's own
    source line is what cargo built. Two spellings of one rule is how they come
    to disagree, so there is one - imported by path, because the filename is
    hyphenated and not importable as a module name.
    """
    path = HERE / "pinned-spec-commit.py"
    spec = importlib.util.spec_from_file_location("pinned_spec_commit", path)
    if spec is None or spec.loader is None:
        print(f"check-engine-floor: cannot load {path}", file=sys.stderr)
        raise SystemExit(2)
    module = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(module)
    except FileNotFoundError:
        print(f"check-engine-floor: {path} is missing", file=sys.stderr)
        raise SystemExit(2)
    return module


def fail(message: str) -> None:
    print(f"check-engine-floor: {message}", file=sys.stderr)
    raise SystemExit(1)


def git(engine: Path, *arguments: str) -> subprocess.CompletedProcess:
    try:
        return subprocess.run(
            ["git", "-C", str(engine), *arguments],
            capture_output=True,
            text=True,
            check=False,
        )
    except OSError as error:
        print(f"check-engine-floor: could not run git: {error}", file=sys.stderr)
        raise SystemExit(2)


def require_commit(engine: Path, revision: str, label: str) -> None:
    completed = git(engine, "cat-file", "-e", f"{revision}^{{commit}}")
    if completed.returncode != 0:
        fail(
            f"{label} revision {revision} is not a commit in {engine}. A shallow clone has only "
            "the tip's history; check carve-rs out with fetch-depth: 0."
        )


def count(engine: Path, from_rev: str, to_rev: str) -> str:
    completed = git(engine, "rev-list", "--count", f"{from_rev}..{to_rev}")
    if completed.returncode != 0:
        fail(f"could not count {from_rev}..{to_rev}: {completed.stderr.strip()}")
    return completed.stdout.strip()


def is_ancestor(engine: Path, ancestor: str, descendant: str) -> bool:
    completed = git(engine, "merge-base", "--is-ancestor", ancestor, descendant)
    if completed.returncode not in (0, 1):
        fail(
            f"could not compare {ancestor} with {descendant}: "
            f"{completed.stderr.strip() or 'git merge-base failed'}"
        )
    return completed.returncode == 0


UNRELEASED_HEADING = re.compile(r"^##\s+\[Unreleased\]\s*$", re.MULTILINE)
NEXT_HEADING = re.compile(r"^##\s+", re.MULTILINE)
# A hex run of 7 or more inside backticks - how this changelog spells a
# revision. Loose enough to catch a short form, anchored on the backticks so a
# hex-looking word in prose is not read as a pin, and case-insensitive so an
# upper-case typo is CAUGHT rather than skipped past.
CHANGELOG_REVISION = re.compile(r"`([0-9a-fA-F]{7,40})`")
ENGINE_MENTION = re.compile(r"carve-rs", re.IGNORECASE)


def check_unreleased_revision(changelog: Path, pinned: str) -> None:
    """The revision the pending section NAMES must be the one the build embeds.

    The published 0.1.0 section named `a33c42ad` while the tag pinned
    `9705274c`, set three minutes before tagging (markup-carve/carve-wasm#50).
    A revision quoted in prose is a second copy of the pin, and the second copy
    is the one nothing checks - the ticket's own summary of how this drifts.
    Only the pending section can be checked here: a released section describes
    the revision that release shipped, which is deliberately not this one.
    """
    try:
        text = changelog.read_text(encoding="utf-8")
    except OSError as error:
        fail(f"{changelog} is not readable: {error}")
    heading = UNRELEASED_HEADING.search(text)
    if heading is None:
        fail(
            f"{changelog} has no `## [Unreleased]` heading, so there is no pending section to "
            "check. Entries land there; the heading moves at release time."
        )
    rest = text[heading.end():]
    following = NEXT_HEADING.search(rest)
    section = rest[: following.start()] if following else rest
    # WHICH quoted hashes are claims about the engine: the ones on a line that
    # says carve-rs. Two failure modes rule this out of being decided any other
    # way. Treating every backticked hex run as an engine revision makes an
    # entry about a commit of THIS repository unwritable; deciding it by
    # resolving each hash in the engine checkout throws a MISTYPED revision away
    # as "not an engine hash" and passes, which is the typo this exists to
    # catch. The line the author wrote it on is the one signal that survives
    # both, and it needs no lookup, so an unreachable checkout cannot soften it.
    claims: list[str] = []
    for line in section.split("\n"):
        if ENGINE_MENTION.search(line):
            claims.extend(CHANGELOG_REVISION.findall(line))
    # A section claiming NO engine revision is fine: not every change is a bump.
    # Claiming one and never naming the pin is the defect - an entry describing
    # a bump to a revision the build does not embed. The superseded revision may
    # be named alongside it ("at X instead of Y"), so the rule is that the pin
    # is among them, not that it is the only one.
    if claims and not any(pinned.startswith(rev) for rev in claims):
        fail(
            f"the [Unreleased] section of {changelog} names carve-rs "
            f"{', '.join(dict.fromkeys(claims))} and never the {pinned} the build embeds. The "
            "prose is a second copy of the pin, and the second copy is the one nothing else reads."
        )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Fail when the engine pin is behind a sibling's.")
    parser.add_argument("--engine", required=True, type=Path, help="path to a carve-rs checkout")
    parser.add_argument("--manifest", required=True, type=Path, help="this package's Cargo.toml")
    parser.add_argument("--lock", required=True, type=Path, help="this package's Cargo.lock")
    parser.add_argument("--sibling-name", required=True, help="the sibling binding, for messages")
    parser.add_argument(
        "--sibling-manifest",
        required=True,
        type=Path,
        help="the sibling's Cargo.toml, the one that names the engine",
    )
    parser.add_argument(
        "--changelog",
        type=Path,
        help="also check that the revision the [Unreleased] section names is the one pinned",
    )
    arguments = parser.parse_args(argv)

    reader = _load_pin_reader()

    if not (arguments.engine / ".git").exists():
        print(
            f"check-engine-floor: --engine {arguments.engine} is not a git checkout of carve-rs.",
            file=sys.stderr,
        )
        return 2

    # The lock is what cargo actually built, so it is the revision this package
    # RUNS. Reading the manifest as well is not redundant: pinned-spec-commit.py
    # fails when the two disagree, and a manifest advertising one engine while
    # the build uses another is its own defect.
    ours_manifest = reader.manifest_revision(arguments.manifest)
    ours = reader.lock_revision(arguments.lock)
    if ours_manifest != ours:
        fail(
            f"{arguments.manifest} pins carve-rs {ours_manifest} but {arguments.lock} resolved "
            f"{ours}. Regenerate the lock and commit it before comparing anything."
        )
    theirs = reader.manifest_revision(arguments.sibling_manifest)

    for label, revision in (("our", ours), (f"{arguments.sibling_name}'s", theirs)):
        require_commit(arguments.engine, revision, label)

    if arguments.changelog is not None:
        check_unreleased_revision(arguments.changelog, ours)

    if ours == theirs:
        print(f"engine floor: pinned at {ours}, the same revision {arguments.sibling_name} embeds.")
        return 0

    if is_ancestor(arguments.engine, theirs, ours):
        ahead = count(arguments.engine, theirs, ours)
        print(
            f"engine floor: pinned at {ours}, {ahead} commits ahead of the {theirs} "
            f"{arguments.sibling_name} embeds."
        )
        return 0

    if is_ancestor(arguments.engine, ours, theirs):
        behind = count(arguments.engine, ours, theirs)
        fail(
            f"the engine pin is {behind} commits BEHIND {arguments.sibling_name}.\n"
            f"  here:            {ours}\n"
            f"  {arguments.sibling_name} embeds: {theirs}\n"
            "Two bindings of one engine render the same document differently for as long as this "
            "holds, and nothing else here reports it: the corpus gate is aimed at the spec commit "
            "the pinned engine pins, so an old pin and an old spec agree with each other. Bump the "
            "rev in Cargo.toml, re-lock, rebuild and re-run the corpus."
        )

    fail(
        f"neither revision is an ancestor of the other:\n"
        f"  here:            {ours}\n"
        f"  {arguments.sibling_name} embeds: {theirs}\n"
        "One of the two is not on the engine's history - an unmerged branch, a rewritten commit, "
        "or a fork. That is a worse finding than lag and it is not cleared by bumping."
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
