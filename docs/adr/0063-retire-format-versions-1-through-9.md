# ADR-0063: Retire format versions 1 through 9; a bump retires its predecessor until the first release

Status: accepted · Date: 2026-10-09 · Applies ADR-0050 · Resolves issue #909 · `FORMAT_VERSION` stays 10

## Context

ADR-0050 lets a version no release has written go at once, decode path and
fixture included.
No release has written any version: issues #537 (first GitHub release) and
#644 (crates.io) are both waiting on the operator.
Versions 6 through 10 landed in eight days, and each kept its predecessor's
decode path: `src/codec.rs` held seven `*_MIN_VERSION` constants, four
literal mixers, a shared length model and a shared offset model that only
frames of an older version read, and `tests/golden/` held fixtures for
versions 3 through 9.
A reader of `decode` carried ten model layouts to follow one.
Issue #909 was filed at `FORMAT_VERSION` 9, proposing to retire 1 through
8; version 10 has since landed, and the same argument retires 9.

## Decision

Versions 1 through 9 are retired under ADR-0050's first condition.
`FORMAT_VERSION` stays 10: nothing a build writes changes, so this is the
retirement CLAUDE.md rule 5 names, not a format change.
`codec::decode` reads exactly one version and takes no version argument;
`decompress`, `decompress_bounded`, `decompress_to_writer` and
`decodes_incrementally` reject a `Method::Lz` frame naming any other
version as `UnsupportedVersion` in one place, `parse_header`.
A `Method::Stored` frame carries no model and still decodes under any
version up to the current one.

Until the first release, a PR that bumps `FORMAT_VERSION` retires the
previous version in the same PR: its decode branch, its gate constant, the
state only it read, and its golden pairs, with each data class a deleted
pair alone covered re-recorded under the new version.
The first release ends this rule, and ADR-0050's release conditions govern
from then on.

## Consequences

A frame a source build wrote below version 10 stops decoding; none was ever
released, and the README already says pre-release frames carry no promise.

`tests/golden/` holds `v10-` pairs for the four data classes the retired
`v3-` pairs pinned (high-entropy binary, long-range repeats, repeated text,
tabular columns through the column expert) beside the existing length
residual pair, so no data class loses its decode pin.

`literal.rs` still holds the retired mixers' encode and decode methods,
`Literal::encode_sse` among them; they now serve only measurement and their
own unit tests, and leave in a follow-up (issue #948).

## Rejected alternatives

**Keep version 9 as a one-step predecessor.** It would keep the shared
length and offset models and one gate pair for frames nobody has, and the
rule above would need an exception for the newest predecessor.
