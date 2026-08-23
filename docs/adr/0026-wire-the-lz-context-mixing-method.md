# ADR-0026: Wire the LZ + context-mixing method

Status: accepted · Date: 2026-08-23 · Resolves `JOURNAL` S2-D2 (partially) · `FORMAT_VERSION` 0 → 1

## Context

M1's port had, until now, produced five modules with no consumer:
`filters`, `lz`, `model`, `literal`, `coder`. Each round-trips on its own
(S2-A2 through S2-A16), and none of them had ever produced a real
bitstream: `decompress` handled only `Method::Stored`. `research/
JOURNAL.md` S2-D2 names the remaining work as wiring all five behind a
new `Method` variant, and ADR-0024 cleared the one open question
(`literal::Literal`'s decode-path `exp` call) blocking it.

That is a bitstream format change: a new method byte is visible in the
wire format (CLAUDE.md hard rule 5), which requires a `FORMAT_VERSION`
bump, this ADR, and decode support for every prior version.

A second question sits underneath the wiring itself and is the real
subject of this ADR: the decoder now consumes attacker-controlled
length and distance fields for the first time in this crate. `docs/
format/SPEC.md` already commits to "decoders never panic and allocate
at most a bounded multiple of the declared output size" and
`docs/TESTING.md` layer 2 already expects declared-size-lie ("bomb")
fixtures. Neither had code to test against before this PR.

## Decision

**1. `Method::Lz = 1`, `FORMAT_VERSION` bumps to 1.** The new method
wires `lz::parse_optimal` (no filter pass yet — `filters` stays
unwired; see Consequences), `model::Model` for the flag/length/offset/
rep-slot streams, `literal::Literal` for literal bytes, and `coder`'s
range coder, in `src/codec.rs`. Ported from the archive's
`encode_body`/`decode`, not the code (ADR-0006). `docs/format/SPEC.md`
carries the payload layout.

**Decode support for version 0 needs no separate code path.** A
version-0 frame only ever contained `Method::Stored` (the only variant
that existed when it could have been written), and `Method::Stored`'s
own encoding is unchanged. `decompress`'s existing check
(`version > FORMAT_VERSION` ⇒ reject) already accepts any version at or
below the current one, so a version-0 `Stored` frame decodes under this
build exactly as it always did. Hard rule 5's "decode support for all
previous versions" is satisfied by construction, not by new code.

**2. The payload declares its own output length, and the decoder never
preallocates from it.** The archive's `decode` trusted a token count
and copied blindly; this port cannot; `rust-craft`'s allocation-
discipline reference names exactly this hazard ("a 200-byte file can
ask for 4 GiB"). `codec::decode` reads a `u32` declared length as the
payload's first field, starts its output `Vec::new()` (not
`with_capacity`), and rejects — `Error::Corrupt`, never a panic — the
instant any token's contribution would grow output past that
declared length. Two consequences fall out of that one rule:

- Total allocation across a single `decode` call is bounded by the
  declared length, a `u32`, regardless of what the token count or any
  individual match/rep length separately claims.
- Total loop iterations are bounded the same way: every token
  contributes at least one byte of output (the smallest legal length is
  1), so a claimed token count far larger than the declared length
  cannot make the loop run longer than the declared length allows —
  the room check fires and returns before that.

**3. Match/rep distances are bounds-checked, not trusted.** `lz::replay`
panics on a distance reaching before the start of output, correctly,
because it only ever replays a token stream `lz::parse_optimal` just
produced (an internal invariant, not adversarial input — `rust-craft`'s
panic-discipline). `codec::decode` reads Rep/Match tokens off an
attacker-controlled bitstream, so it cannot reuse that function as-is;
`codec::copy_checked` is `replay`'s `copy_match` with the same
overlapping-run technique, `checked_sub` in place of the panicking
subtraction, and `Error::Corrupt` in place of `.expect(...)`.

**4. The rep cache and the length/offset bucket function are shared,
not re-derived.** `lz::RepCache` and `lz::bucket` (plus the
`LENGTH_BUCKETS`/`OFFSET_BUCKETS` alphabet sizes) are now `pub(crate)`
and used directly by `codec`. `JOURNAL` S1-A3 records the founding port
bug as exactly a rep-cache/bucket confusion between two hand-written
copies of the same rule; reusing one implementation on both the encode
and decode side removes the class of bug, not just this instance of it.

**5. `compress` picks `Method::Lz` whenever it is smaller, `Stored`
otherwise.** Matches `docs/format/SPEC.md`'s Stored-floor invariant.
Inputs longer than `u32::MAX` bytes (the declared-length field's width)
skip `Method::Lz` entirely rather than panicking in a public API; nothing
in this crate exceeds that bound today, and a future streaming/block API
is the intended way to lift it, not a wider field here.

## Consequences

M1's checklist item ("wiring all of it... behind a new `Method`
variant") is not fully closed by this PR: filter selection
(`filters::select::pick`, trial-encoding against candidate filters) is
not wired in. `Method::Lz` always runs on raw input. That is a
deliberate slice, not an oversight — trial-encoding against several
filter candidates is a distinct piece of complexity from entropy-coding
wiring, and the smallest useful slice that produces a real bitstream
doesn't need it yet. `research/JOURNAL.md` S2-D2 keeps this as its
remaining scope.

`docs/format/SPEC.md`'s invariants section previously stated
"integer-only probability arithmetic in the coded path" as the
determinism mechanism, citing S1-A5. That predates ADR-0024, which
resolved the actual mechanism as "no libm transcendental on the decode
path" (IEEE-754 basic operations are already reproducible; only the
transcendental family isn't). This PR corrects that sentence in the same
change that first makes it load-bearing (`Method::Lz` is the first
method whose decode path actually runs `literal::Literal`'s mixer) —
verifying a claim against a run instead of trusting a stale doc, the
house value this project keeps naming and occasionally not doing.

This is the first M1 slice that produces a real bits/byte number.
`research/JOURNAL.md`'s S2-A17 entry records it, on the named corpus
`research/imports/session-1/mothergod.rs` (25,524 bytes), the same file
ADR-0024's accuracy test already uses.

## Alternatives rejected

**Wire filters in the same PR.** Would fully close the M1 checklist
item in one PR, at the cost of a much larger diff mixing two concerns
(trial-selection control flow, entropy-coding wiring) that fail
independently and should be reviewable independently. `CLAUDE.md`'s
"small PRs, one idea per PR" rule and this project's own established
cadence (S2-A2 through S2-A16, one module per PR) both point the same
way.

**Trust the declared output length and preallocate from it.** Simpler
code, and exactly the hazard `rust-craft`'s allocation-discipline
reference exists to name. Rejected outright, not weighed.

**Reuse `lz::replay` for decode instead of a checked copy.** Would
either require loosening `replay`'s panic (weakening a guard that is
correct for its actual caller, `parse_optimal`'s own tests) or wrapping
every call in `catch_unwind` (turns an input-validation problem into an
unwind-safety problem, and `CLAUDE.md` rule 2 wants a `Result`, not a
caught panic).
