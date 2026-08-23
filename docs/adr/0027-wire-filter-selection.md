# ADR-0027: Wire filter selection into `Method::Lz`

Status: accepted · Date: 2026-08-23 · Resolves `JOURNAL` S2-D2 (in full) · `FORMAT_VERSION` 1 → 2

## Context

ADR-0026 wired `lz`, `model`, `literal`, and `coder` behind `Method::Lz`,
deliberately leaving one thing out: `filters::select::pick`'s trial
selection never ran, so every frame filtered `data` through the identity
transform only. `research/JOURNAL.md` S2-D2 named that as the remaining
M1 scope, and ADR-0026's own "Alternatives rejected" section said why it
was split out: two concerns (trial-selection control flow,
entropy-coding wiring) that fail independently and review better apart.

`filters::select::pick` (S2-A7) already shortlists which filters —
delta, BCJ, transpose, or none — are worth a full trial encode against a
given input, using a bounded order-1 entropy proxy. `filters::delta`,
`filters::bcj`, and `filters::transpose` (S2-A2 through S2-A6) already
provide reversible `encode`/`decode` pairs. Nothing yet ran `pick`'s
output through a real trial encode or wired the winner into a real
bitstream.

## Decision

**1. `codec::encode` trials every candidate `pick` shortlists, keeps the
smallest.** For each `Candidate`, `codec::apply_filter` transforms
`data`, then `codec::encode_tokens` (what `encode` used to be, in full,
before this PR) runs the existing LZ + context-mixing pipeline on the
filtered bytes. Whichever candidate's `encode_tokens` output is smallest
wins; ties keep whichever `pick` ranked first (`Candidate::Identity` is
always among the candidates, so this never regresses below what
ADR-0026 already shipped). Filters are trialed against the raw input
directly, never stacked, matching the archive's `encode`.

**2. The winning candidate is a 2-byte prefix on the payload, not a
1-byte packed code.** `[kind, param]`: `kind` is 0 (Identity), 1
(Delta), 2 (Bcj), or 3 (Transpose); `param` is the delta stride or
transpose column count, zero for the two filters that take none. The
archive packed this into one byte with reserved ranges (`0` = identity,
`1..=96` = delta stride, `97` = BCJ, `100..=113` = transpose column
index). This port uses an explicit tag instead: it does not need to
duplicate `filters::select`'s private `TRANSPOSE_COLUMNS` table to
recover an index from a column count, and it makes an invalid byte pair
(`[0, 5]`, `[1, 0]`) structurally distinguishable from a valid one
without consulting that table at decode time either.
`Candidate::to_header_bytes`/`from_header_bytes` (`src/filters.rs`) own
the mapping, next to the enum, so there is exactly one place that knows
it — the single-source-of-truth this project asks for standingly, not
just at introduction.

**3. `FORMAT_VERSION` bumps to 2, and version-1 `Method::Lz` decode is
dropped explicitly, not silently.** The 2-byte prefix moves the payload
layout ADR-0026 shipped: what version-1 encode wrote as
`[declared_len][token_count][coded bytes]` starting at payload byte 0,
version-2 encode writes as `[filter_selector][declared_len][token_count]
[coded bytes]`. A version-1 `Method::Lz` frame fed to this build's
`codec::decode` would misread its first two bytes as a filter selector
rather than the top of `declared_len`. Rather than rely on `decode`'s
adversarial-input defenses to fail safely by coincidence on that
misread, `codec::LZ_MIN_VERSION` (2) makes the rejection explicit:
`decompress` returns `Error::UnsupportedVersion` for any `Method::Lz`
frame naming a version below it, before `codec::decode` runs at all.
`Method::Stored` needs no equivalent floor — its payload is untouched by
this change, exactly the argument ADR-0026 made for version 0.

Dropping version-1 `Method::Lz` decode support outright (hard rule 5's
"unless an ADR drops one") is safe here specifically because nothing has
shipped: `Method::Lz` landed the same day as this PR (ADR-0026, `main`
history), there is no 0.1 release (ROADMAP M6, not yet reached), and no
persisted frame exists outside this repository's own tests. A real
version-1 archive would force this ADR to keep a second decode path
instead.

**4. Filters preserve length, so `declared_len` needs no format
change.** Every filter in `src/filters.rs` (`delta`, `bcj`, `transpose`,
and `Identity`) returns exactly as many bytes as it consumed, so the
declared-output-length field already means "length of the filtered
bytes `codec::decode` reconstructs," and that is always `data.len()`
regardless of which candidate won. `decode` reverses the filter
(`codec::undo_filter`) only after reconstructing those bytes and
confirming `output.len() == declared_len`, so a corrupt intermediate
stream is still caught by the existing length check before any filter
code sees it.

## Consequences

`research/JOURNAL.md` S2-D2 closes in full: M1's checklist item ("port
`mothergod.rs` into `src/`... behind the frame format") no longer has
open scope. Measured on the named corpus
`research/imports/session-1/mothergod.rs` (25,524 bytes, the same file
ADR-0026 measured): 2.3184 bits/byte, `Candidate::Identity` selected —
unchanged from ADR-0026's 2.318 bits/byte, because this file is
structured Rust source text, not the columnar/binary shape `delta` or
`transpose` win on (`JOURNAL` S1-R1 already predicted delta loses on
text). A real bits/byte win from this PR needs a corpus with that shape;
`bench/`'s structured generators (`access_log`, `json_records`) and the
eventual Silesia/Canterbury fetch (S2-D1) are where that shows up, not
this file. `codec::tests::roundtrip_columnar_drift_data_uses_a_non_identity_filter`
proves the wiring picks and correctly reverses a non-identity filter
end to end, on synthetic columnar data built the same way
`filters::select::tests::pick_selects_delta_for_columnar_drift` already
proves `pick` ranks it first.

Trial encoding costs what it always would: one full `lz::parse_optimal`
+ range-encode pass per candidate `pick` returns (typically 2, up to 4).
`pick`'s own cost stays bounded by its `PROBE_LEN`/`BCJ_SCAN_LEN`
probes, unchanged by this PR. No streaming API exists yet (ROADMAP M4),
so this cost was already inherent to `Method::Lz`'s design, not
introduced here.

## Alternatives rejected

**Reuse the archive's packed single-byte filter id.** Saves one payload
byte per frame. Rejected: recovering a transpose column count from a
packed index requires either exposing `filters::select`'s private
`TRANSPOSE_COLUMNS` table to `codec.rs` (two modules needing to agree on
one array's contents and order, a duplicate source of truth) or moving
the array itself into a place both modules see. The 2-byte tag keeps the
byte-to-`Candidate` mapping next to the enum it serializes and needs no
shared table.

**Keep `FORMAT_VERSION` at 1 and reuse it for the new layout.** Would
mean a version-1 frame's `Method::Lz` payload layout is ambiguous
without external metadata (did this encoder predate or postdate the
filter wiring?) — exactly the hazard `FORMAT_VERSION` exists to name
away. Rejected outright, not weighed.
