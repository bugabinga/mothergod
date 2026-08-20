# Corpus policy

A benchmark is a claim about which data matters; whoever composes the corpus
crowns the winner (JOURNAL S1-L2, S1-L7). These rules keep us honest.

## Structure

- **Train slices** — rotating windows over each dataset; a different window
  every iteration so offsets can't be memorized.
- **Sealed validation set** — different seed AND different datasets from
  train. Experiments are accepted only with train improvement and no
  validation regression. No agent ever tunes against it.
- **Held-out finals** — Silesia and Canterbury (and later enwik8), fetched at
  pinned revisions, whole files. Quoted in README/BENCHMARKS only from these.

## Mandatory datasets

- The entropy ladder: iid sources at H₀ = 1, 2, 4, 6, 8 bits — verifies the
  coder tracks the theoretical floor and prices our own cleverness (context
  machinery must not tax structureless data beyond a stated budget).
- markov-H8/2 trap: uniform byte histogram, conditional entropy 2.0 —
  separates context modelers from histogram coders.
- zipped/random rows — the pigeonhole check; stored-block floor must hold.

## Growing the corpus

Adversarial additions are welcome and scored by **regret** = (our bits/byte)
− (reference compressor bits/byte) on the same data. Additions need positive
regret — data we are *relatively* bad at. Pure noise is hard for everyone,
has zero regret, and is auto-rejected. Reference compressors: zstd -19 and
xz -9e at pinned versions.

## Sourcing: borrowed and our own

We use both, for different jobs.

**Borrowed (comparability).** Nobody believes a compressor that only wins on
its own corpus. Standard corpora are how our numbers become comparable to
zstd/xz/brotli papers and to other people's runs:

- **Silesia** — the modern general-purpose standard (zstd/lzma tuning
  ground); our primary held-out final.
- **Canterbury** — the classic small-file suite; cheap, everyone quotes it.
- **enwik8** (later enwik9) — the large-text standard (LTCB/Hutter);
  relevant once large windows land (M3+).
- Calgary — historical interest only; optional.

Borrowed corpora are **never committed** (size and third-party copyright).
`bench/corpus.toml` pins each file by URL + SHA-256; the harness fetches and
caches, and refuses to run on a checksum mismatch. Pinned checksums make the
corpus reproducible without redistributing it.

**Our own (honesty and coverage).** Standard corpora are public — every
modern compressor is implicitly tuned on them, including ours-by-imitation.
Our generators probe what they miss, are deterministic (seeded, in Rust,
in-repo), and cost nothing to store:

- the entropy ladder and markov-H8/2 trap (mandatory, above);
- structured generators covering the founding session's classes: jsonl/log
  records, json, base64-wrapped payloads, interleaved 16-bit audio, gradient
  image, sqlite-like records, x86-dense binaries (specs in the founding
  corpus, `git show 1a3b1c8:research/imports/session-1/corpus.py` — port
  behavior, not code);
- an **adversarial decode corpus**, committed in-repo (tiny files):
  truncations, bit-flips, bombs, wrong versions, fuzz-found crashers — this
  one is for the never-panic suite, not for ratio numbers (see
  `docs/TESTING.md`).

**Sealing.** Three tiers, strictly separated: rotating train slices (agents
optimize against these), the sealed validation set (accept/reject gate —
generators use held-out seeds AND held-out dataset kinds; no agent ever
tunes against it), and held-out finals (Silesia/Canterbury/enwik8 whole
files — quoted in README/BENCHMARKS, run at milestones, never inside the
experiment loop).

## Quoting numbers

Every published number names: corpus + revision, slice size, codec version,
and cost basis (real bitstream vs model cost). Model-cost numbers are marked
as such — the founding session's ~0.1% optimism between ideal AC cost and the
real coder is documented and must stay measured.
