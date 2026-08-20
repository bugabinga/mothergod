# Contributing

Thanks for your interest! mothergod accepts contributions from humans and
agents through the same pipeline. This page is written for humans; agents are
governed by [`CLAUDE.md`](CLAUDE.md) instead.

## The short path

1. **Questions / ideas / bugs** → [open an issue](../../issues/new/choose).
   An agent triages daily. Mention `@claude` anywhere to get an interactive
   answer.
2. **Code** → fork, branch, PR. Expect your review to come from an AI
   reviewer that will actually run your claims. That's normal here.

## What to know before writing code

- Read [`research/JOURNAL.md`](research/JOURNAL.md) first if you're touching
  the codec. Many plausible ideas were already tried and falsified; the
  journal saves you the repeat. If you want to retry a rejected idea, say
  which condition changed.
- Quality gates (CI runs exactly these):

  ```sh
  cargo fmt --check
  cargo clippy --all-targets -- --deny warnings
  cargo test --all-targets
  cargo test --doc
  ```

- Lossless is non-negotiable: codec changes ship with round-trip tests.
- The decoder must never panic on any input — compressed data is adversarial.
- Benchmark claims must name their corpus (see
  [`research/corpus/POLICY.md`](research/corpus/POLICY.md)).
- Keep PRs small: one idea per PR, `CHANGELOG.md` updated for anything
  user-visible.

## Review expectations

The reviewer agent is deliberately adversarial — it tries to refute your
change before approving it. It is instructed to be courteous to humans, to
explain its reasoning, and to escalate to the human operator
([@bugabinga](https://github.com/bugabinga)) when a disagreement isn't
resolving. If a review feels wrong, say so in the thread or open an issue;
appeals go to the operator (see [`GOVERNANCE.md`](GOVERNANCE.md)).

## Conduct

Be kind. See [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md). Conduct issues are
handled by the human operator, never by agents.
