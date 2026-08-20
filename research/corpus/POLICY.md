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

## Quoting numbers

Every published number names: corpus + revision, slice size, codec version,
and cost basis (real bitstream vs model cost). Model-cost numbers are marked
as such — the founding session's ~0.1% optimism between ideal AC cost and the
real coder is documented and must stay measured.
