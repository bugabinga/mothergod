# Proptest strategies for codec types

Applies to the layer 1 properties in TESTING.md; proptest adoption is
issue #452's scope. The strategy is the test: a property over a bad
generator proves nothing about the inputs that matter.

## Construct, never filter

`prop_filter` discards draws and cripples shrinking. Build valid
values directly:

- an offset valid for a window: generate an index and map it into
  range (`min`/`%`), never filter out-of-range draws;
- a token stream: `prop_oneof!` over the token kinds, with
  `prop_compose!` building each kind's fields, so illegal tokens are
  unrepresentable in the generator the way types make them
  unrepresentable in the codec;
- a dependent pair (a length bounded by a position): `prop_flat_map`
  from the first to the second, accepting its weaker shrinking, only
  when no direct construction exists.

## Generate the distribution the codec targets

Random bytes exercise the entropy coder and nothing else. Draw from:

- the corpus policy's generator classes
  (`research/corpus/POLICY.md`), so properties sweep the data the
  codec is for;
- pure noise, as the incompressible control;
- layer 1's edge sizes as explicit `Just` cases inside the strategy,
  not as hoped-for random draws.

## Shrinking is the product

A property failing on a 64KB blob is a finding; the same failure at 3
bytes is a diagnosis. Prefer `prop_map` over `prop_flat_map`; build
from scalar strategies so every component has a path down; verify a
new strategy shrinks by planting a temporary failure and reading the
minimal case it reports.

## Profiles and persistence

- Case counts come from the environment (`PROPTEST_CASES`): the PR
  gate runs the fast profile, the weekly tier runs large, and the
  tier table in TESTING.md owns that cadence.
- Commit `proptest-regressions/`: every found failure replays first
  on later runs. It is the regression suite proptest writes for you.
- A meaningful minimal case gets promoted to an example test named
  after the bug (the ladder's rung 1), because a seed-file line is
  not documentation.
