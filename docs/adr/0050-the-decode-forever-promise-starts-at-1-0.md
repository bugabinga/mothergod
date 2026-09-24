# ADR-0050: The decode-forever promise starts at 1.0

Status: accepted · Date: 2026-09-24 · Supersedes ADR-0041 · Prompted by issue #708 (operator question, Telegram, 2026-09-23)

## Context

ADR-0041 froze `docs/format/SPEC.md` on 2026-09-01 and promised that
every wire version from 2 on decodes forever. No release has shipped
(issues #537 and #644 wait on the operator), so no frame outside
`tests/golden/` exists in any of those versions. Four wire versions
landed in four weeks: 2 (ADR-0028), 3 (ADR-0038), 4 (ADR-0046); the
freeze came three days after version 3.

The promise has a concrete cost today. `src/codec.rs` dispatches on
`LITERAL_SSE_MIN_VERSION` at two sites to keep the direct 256-way
literal path alive for one fixture, `tests/golden/v2-lz-repeated-text.mgdc`,
that no build anyone installed can have written. README.md calls the
library API "subject to change until 1.0"; the format promise was not
aligned with it.

ADR-0041 ruled that a live research lead does not gate a freeze, because
a new model class lands as a new version. It did not ask when frames
worth protecting start to exist. zstd's format promise started at 1.0,
and its pre-1.0 frames became legacy input behind optional support.

## Decision

The decode-forever promise starts with the first version a 1.0 build
writes. From that version on, no version is ever retired: ADR-0041's
discipline, restarted there.

Before that, a version is retired by its own ADR under one of two
conditions:

- **No release has written it.** It goes at once, decode path and
  fixture included.
- **A release has written it.** It goes only after a later release that
  still reads it and writes its successor has shipped, and `CHANGELOG.md`
  has named the retirement, so a user can re-compress first.

"Written" means a release's encoder produced frames of that version. A
release that only reads a version does not protect it.

What ADR-0041 established otherwise stands: `docs/format/SPEC.md` is
normative for the current code and changes in the same PR as the code;
every version change carries an ADR and a golden fixture (CLAUDE.md rule
5, `tests/golden.rs`); a live research lead gates nothing. The spec's
status paragraph is the one statement of the retirement rule. CLAUDE.md
rule 5 points at it, README.md and `site/index.html` restate it in plain
words, and `tests/claims.rs` holds that restatement to the release state
`CHANGELOG.md` records.

Version 2 is retired under the first condition, in the maintainer's PR
that follows this ADR (issue #708): the direct 256-way literal path
(`Literal::encode`/`decode`), its two dispatch sites,
`LITERAL_SSE_MIN_VERSION` and the version-2 fixture go, and
`LZ_MIN_VERSION` becomes 3. Version 3 stays: its decode path is one
constant and one gate on the column expert, and its four fixtures are
the decode pins for the three data classes version 4 codes identically,
so retiring it now would regenerate four fixtures to remove five lines.

## Consequences

For a user: a frame written by a source build before the first release
has no promise, and the README says so. A frame written by an 0.x
release stays readable until a changelog entry names its retirement,
with a release in between to re-compress. From 1.0, forever.

For the project: a wire mistake found before 1.0 is fixed by deletion
rather than carried indefinitely, and each pre-1.0 version costs its
decode path only until an ADR retires it. The price is one migration
step per version an 0.x release wrote: an extra release and a changelog
line.

`FORMAT_VERSION` stays 4; this ADR touches nothing on the wire. The spec
loses the word "frozen" until 1.0. Hard rule 5 loses its version-2 floor.
ROADMAP.md's M6 promise reads "a versioned format" rather than "a frozen
format".

## Rejected alternatives

**Keep ADR-0041 and delete nothing.** Honest only if frames exist to
protect. None do, and the cost is a permanent decode path per
pre-release version, at the current rate one a week.

**Legacy decode behind a feature flag, zstd's shape.** A second build
configuration to test, for frames nobody has. Deletion is smaller, and a
version an 0.x release wrote gets a migration release instead.
