# ADR-0061: Code copy-length residual bits through adaptive trees

Status: accepted · Date: 2026-10-08 · Resolves `JOURNAL` S2-A114 · `FORMAT_VERSION` 9 → 10

## Context

A copy's length is a `lz::bucket` symbol plus the residual bits below that
bucket, and until version 9 those bits went out raw: a value in bucket `b`
always cost `b` flat bits however predictable it was.
`research/JOURNAL.md` S2-A114 measured LZMA's remedy, an adaptive binary
tree over the top four residual bits per (bucket model, bucket), on the
token stream of the filter `encode` selects, for lengths and distances
together.
It was accepted unwired, and left one decision to this slice: whether the
distance half ships.
Its own post-verdict diagnostic put the distance trees at +0.012368 b/B on
train, so train alone, the only evidence a choice may use, says no.

## Decision

From `FORMAT_VERSION` 10 (`codec::LENGTH_RESIDUAL_MIN_VERSION`) the top
four residual bits of every copy length code through a `ResidualTree`
beside its bucket model, one tree set for `Token::Match` lengths
(`Models::length_match_residual`) and an independent one for `Token::Rep`
(`length_rep_residual`); bits below those four stay raw.
A distance's residual stays raw at every version.
The gate is version alone, every candidate, the same shape as
`LENGTH_SPLIT_MIN_VERSION`; versions 3 through 9 decode through the old
path unchanged.
The S2-A114 apparatus (paired sink, state, experiment entry point) is
deleted in the same PR, as the `compression-experiment` skill requires.

## Consequences

Measured on real bitstreams (`research/JOURNAL.md` S2-A115): train mean
**-0.050196 b/B on `bench::baseline`'s 11 train cases**, 10 of 11 improved,
none regressed; sealed `access_log` and `gradient_image` both improved.
Decode cost moved +1.17 calibration steps per byte on one runner, inside
its run-to-run spread, so `speed::BASELINE_DECODE_COST` stays.
`Models::try_new` allocates the tree nodes too, each fallibly (hard rule 2).
`tests/golden/v10-lz-length-residuals` pins the path; every older fixture
stays, decode-only.
`bench/baseline.json` and the two finals reports are not regenerated here:
`baseline_gate check` passes against the unchanged baseline, and a new one
retires both reports' fingerprint, which needs their network fetches.

## Rejected alternatives

**Ship the distance trees too, as S2-A114 priced them.** Rejected on train:
they cost +0.012368 b/B where the length trees alone gained, and the only
case for them was sealed `access_log`, a number no variant may be chosen
on (`research/corpus/POLICY.md`).
A distance half shaped like LZMA's, low align bits instead of the top ones,
is a separate candidate.
