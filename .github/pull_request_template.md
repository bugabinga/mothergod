<!-- The adversarial reviewer agent will try to refute this PR by executing
     its claims. Filling this in honestly makes that fast. -->

## What & why

<!-- One idea per PR. Reference ROADMAP items / journal ids / issues. -->

## How it was verified

<!-- Commands you actually ran. Unverified claims will be flagged. -->

## Checklist

- [ ] Quality gates pass locally (`cargo x check`)
- [ ] Codec change → round-trip tests included, decoder safe on adversarial input
- [ ] Numbers quoted → corpus named (see `research/corpus/POLICY.md`)
- [ ] Experiment → `research/JOURNAL.md` + `research/progress.jsonl` entries
- [ ] Format change → `FORMAT_VERSION` bump + ADR + `docs/format/SPEC.md`
- [ ] User-visible change → `CHANGELOG.md` (Unreleased)
