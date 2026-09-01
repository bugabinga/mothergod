# ADR-0041: Freeze the format spec

Status: accepted · Date: 2026-09-01 · Prompted by issue #422, ROADMAP M4

## Context

`docs/format/SPEC.md` has carried a "DRAFT... unstable" status since M0,
gating it on a ROADMAP M4 line that read "Frozen format spec v1 +
`FORMAT_VERSION` 1." `FORMAT_VERSION` is 3 (bumped for `Method::Lz` at 1,
the LZ payload layout at 2, SSE literal coding at 3, each a real,
needed evolution); the wire byte cannot honestly go back to 1, so that
line could not mean what it said. Issue #422 traced the "1" to the
spec document's own status, fossilized from before the format needed
to move past its first cut, and asked two questions before the
milestone item could be worked as a slice: whether the format is
actually ready to freeze, and what freezing should mean if so. #424
corrected the ROADMAP wording; this ADR rules on the substance.

CLAUDE.md hard rule 5 already requires an ADR plus decode support for
every previous version on any format change, but lets an ADR drop
support for an old one — the escape hatch ADR-0026/ADR-0028 used to
retire version 1's incompatible `Lz` payload. `research/JOURNAL.md`
S1-P8 (GLN-style predictors) is a live standing lead that could plausibly
want a new `Method` or version. Waiting for the research program to run
out of live leads before freezing is an infinite regress: the journal
always carries one.

## Decision

`docs/format/SPEC.md` is stable, frozen at `FORMAT_VERSION` 3. Every
version the frozen spec documents — 2 and 3, the two `tests/golden/`
still pins — decodes forever from this point on: no future ADR may
drop decode support for either one. Version 1 stays retired; its
drop (ADR-0026/ADR-0028) predates this freeze and is not reopened.

A live research lead does not gate the freeze. A model-class change
(S1-P8 or anything else) lands as a new `FORMAT_VERSION` under hard
rule 5 regardless, so freezing now costs nothing against future work —
it only forecloses *retiring* a version, never *adding* one.

CLAUDE.md hard rule 5's "unless an ADR drops one" carve-out no longer
applies to version 2 or later: format evolution from here continues
solely by adding a new version via a `FORMAT_VERSION` bump, never by
dropping one already covered by the frozen spec.

## Consequences

Every wire version starting at 2 is a permanent decode commitment: a
decoder built against this crate keeps decoding a version-2 or
version-3 frame no matter how many versions ship after it. That is
the product promise a general-purpose compressor sells, so it lands
now rather than at some later comfort point (issue #422's ruling).

Cost: a design mistake discovered in version 2 or 3's wire layout can
no longer be fixed by dropping the version, only by adding a corrected
one and carrying the old decoder path alongside it indefinitely. Rule
5's ADR-plus-previous-version-support discipline already made dropping
a version rare and deliberate (one instance in the project's history);
the freeze removes the last exit from that decision entirely.

`docs/format/SPEC.md`'s status line and title update in the same PR as
this ADR. `ROADMAP.md` M4's frozen-format-spec item is checked off in
the same PR.
