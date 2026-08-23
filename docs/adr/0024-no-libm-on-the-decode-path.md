# ADR-0024: No libm on the decode path

Status: accepted · Date: 2026-08-23 · Resolves `JOURNAL` S2-D3 · Unblocks S2-D2

## Context

`JOURNAL` S2-D3 has blocked M1's `Method`-wiring slice through two
heartbeats. It asks whether `literal::Literal`'s exponentiated-gradient
weight update should stay `f64` or be rebuilt in fixed point, and it
offers two resolutions: reconstruct the integer version, or write an ADR
accepting `f64` with a stated mitigation.

Three things about that framing are wrong, and each one made the blocker
look bigger than it is.

**The direction was already decided.** `JOURNAL` S1-A5 records the
integer-only probability path as accepted architecture, for exactly this
reason: it "retired the cross-platform f64 determinism hazard". S2-D3's
option (b) proposes to un-accept it. What was actually missing was never
the decision, it was an artifact: the integer refactor postdates
`research/imports/session-1/mothergod.rs`, so the port had nothing to
copy and copied the float version. A missing artifact got recorded as an
open question, and an open question is something nobody is assigned to
close.

**Two hazards of different severity were braided into one item.** Where
a float sits decides what a divergence costs, and only one of the two
costs correctness:

| float site | who computes it | a divergence costs |
|---|---|---|
| `literal.rs` mixing-weight update | encoder **and** decoder | desync, corrupt output: a lossless violation (hard rule 1) |
| `lz.rs` prices and DP costs | encoder only | a different, still-valid frame every decoder reads: reproducibility |
| `filters.rs` entropy scoring | encoder only | same as above |

Verified 2026-08-23: `decompress` handles only `Method::Stored`, and
`parse_optimal`/`pick` have no non-test callers, so none of this is live
today. That ends with the `Method` wiring, which is why the decision is
due now and not later.

**The hazard is one function call, not a class of arithmetic.** IEEE 754
requires `+`, `-`, `*`, `/` and `sqrt` to be correctly rounded, and Rust
gives IEEE-754 semantics with no implicit FMA contraction, so those
operations are bit-reproducible across platforms. The transcendental
functions are not: they are libm's, and libm implementations disagree in
the last ulp. The whole crate contains four transcendental calls:

- `literal.rs:293` `gradient.exp()`, on the decode path. This one, alone,
  is S2-D3.
- `filters.rs:773`, `filters.rs:807`, `lz.rs:520`, all `log2`, all
  encoder-only.

So "port the mixer to fixed point" was never the blocking work. Replacing
one `exp` was.

## Decision

**1. Nothing on the decode path calls a libm transcendental function.**
Any value the encoder and decoder must both compute is built from integer
arithmetic or from IEEE-754 basic operations, and from nothing else. This
is a hard rule with the same standing as "the decoder never panics": it
guards hard rule 1, because two machines that disagree about a mixing
weight produce a frame that does not round-trip.

S2-D3's option (b) is rejected in the form of restricting supported decode
platforms. That trades the mission's "deterministic across platforms" for
implementation convenience, and the Mission section is not mine to amend
(ADR-0011). Its other form, a vendored `exp` that computes in basic
operations only, is not a mitigation for a violation. It satisfies the
rule outright, and it is the expected fix.

**2. The rule is enforced by clippy, not by memory.** The implementing PR
adds `clippy.toml` with `disallowed-methods` covering `f64::exp`, `ln`,
`log2`, `log10`, `powf`, `powi`, and the trigonometric family, plus their
`f32` twins. The three encoder-only sites then carry
`#[allow(clippy::disallowed_methods, reason = "encoder-only: ...")]`,
which is the justification comment CLAUDE.md's style rule already
requires of every `#[allow]`. The default inverts: a float transcendental
is a build failure until someone writes down why their site is
encoder-only. That is the point. A future contributor adding an `exp` to
the decode path gets a red check, not a corrupted archive two years from
now (ADR-0022: this is hot, it is silent when it breaks, and clippy is a
hard substrate with a liveness signal CI already runs).

**3. Encoder-only floats stay, and stop blocking anything.** `lz.rs`
prices and `filters.rs` scoring keep their `log2`. A platform that prices
differently emits a different frame, and every decoder reads it. What that
costs is bit-identical reproducibility, which golden frames want (M4,
`docs/TESTING.md` layer 5) and which the M2 benchmark gate will read as
noise between runners. It does not cost correctness, so it does not block
M1. It becomes its own journal entry, decided when golden frames are
built.

**4. Integer-only remains the destination, demoted to a speed lead.**
S1-A5 claimed two wins for the integer path: determinism and 1.5-4x speed
with autovectorization. Decision 1 buys the determinism now, at the cost
of one function. The speed half is real and unmeasured in this codebase,
so it belongs to M5 with the other speed work, where it can be judged
against a benchmark instead of asserted from a transcript. Nothing about
this ADR forbids the full integer mixer. It stops being a prerequisite.

## Consequences

M1's critical path reopens. S2-D2's `Method` wiring was waiting on a
decision that was, on inspection, already made and much smaller than its
description, and the maintainer spent two heartbeats on M2 corpus work
while the project's top outcome metric could not move. RATIO stays
unmeasurable until a real bitstream exists, so this is the unblock that
matters most this week.

The acceptance test for the replacement `exp` is not bit-identity with
the archive. Quantizing or approximating the update changes predictions
slightly by construction, so demanding identical output would fail a
correct implementation. It is two claims instead: the mixer round-trips
exactly, and its bits/byte on a named corpus stays within 1% of the `f64`
mixer it replaces. The `f64` version is kept as a test-only reference to
diff against. If the experiment shows 1% is the wrong budget, the number
is revisable on that evidence; what is not revisable is stating a budget
before the measurement instead of blessing whatever comes out.

A residual platform risk is named rather than assumed away: a 32-bit x86
target using x87 with excess intermediate precision could still diverge on
basic operations. Current i686 Rust targets use SSE2 for `f64`, so this is
narrow, and the honest verification is golden frames across targets in M4
(layer 5), not an argument in this file.

## Alternatives rejected

**Rebuild the mixer in fixed point now.** The work S2-D3 actually named,
and it is a rewrite of the numerics on M1's critical path, justified by a
speed claim this codebase has never measured. Decision 4 keeps it, later,
where a benchmark can judge it.

**Accept `f64::exp` and restrict decode platforms.** A compressor whose
archives decode on the machine that wrote them is not a compressor.

**Write the rule in CLAUDE.md and trust review to catch violations.** The
rule is mechanically checkable, and a mechanically checkable rule enforced
by reading is a rule that holds until the first tired reviewer. Hard rule
3 puts verification outside the proposer; clippy is outside everybody.

**Ban floats from the crate entirely.** Simpler to state and wrong. It
would force the optimal parse and filter scoring into fixed point for a
property neither one needs, which is the exact conflation that made S2-D3
look like a rewrite.
