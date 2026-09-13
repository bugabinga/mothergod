---
name: workflows
description: Use when writing or reviewing a file under .github/workflows/ or .github/actions/, pinning or bumping an action, adding caching, changing a job's permissions or triggers, spending runner minutes, or when a run is red, slow, or never started.
user-invocable: true
---

# Building a CI workflow that is worth having

A workflow is a program that runs with this project's secrets, on a machine
nobody here owns, triggered by things other people can cause. Most advice about
Actions is about YAML. Almost none of what goes wrong is YAML.

Every number lives in [GitHub's reference pages][billing], linked and not
copied here: limits, prices and plan allowances change, and a document that
asserts one is a document that will be wrong without anybody noticing. What is
here is the judgement, which does not.

The workflows in this repository are the agent system itself, so a change here
is a change to how every other seat behaves. That is the whole reason this is a
skill and not a checklist somebody remembers.

## Before the first line

1. **What occasion is this?** A trigger is an event with a payload and a trust
   level, not a schedule. A fork's `pull_request` has a read-only token and no
   secrets; [`workflow_run` and `pull_request_target`][events] have both, for
   any head.
   If you cannot say in one sentence who can cause this to run, stop.
2. **What may it do?** Start at `permissions: {}` and raise per job, which is
   what `agent-bdfl.yml` does. The [full key list][syntax] is short; read it
   rather than guessing, because the keys do not partition the way their names
   suggest. The default is what an attacker inherits, and a
   job-level block **replaces** the workflow-level one rather than extending
   it: anything unlisted becomes `none`, including a `contents: read` declared
   at the top of the file. `codeql.yml`'s `changes` job carries the comment
   explaining why both of its scopes are load-bearing; that is the shape to
   copy.
3. **What is its output?** A check name, an artifact, a comment, a branch. Name
   it now. A required check's _name_ is the contract with the ruleset, so
   renaming a job wedges every merge until the rule is renamed with it. This
   repository requires exactly `fmt`, `clippy`, `test`, `doc` and `ratio`
   (CLAUDE.md "Commands"); `stalled-prs` hardcodes the same list, and a gate
   renamed in one place reads as `never-fired` on every PR.
4. **What does it cost?** See _Minutes_. The operator's standing directive is
   that a workflow minute is subscription and contributor wait, both budget.

## Pin every action to a full commit SHA

```yaml
- uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7
```

A SHA is [the only immutable way to name an action][secure]; a tag is a
pointer a compromised account can move. The trailing comment is what makes the pin
readable, so keep it true when you bump. Dependabot maintains these and
rewrites the comment, but it has no ecosystem for pinned _tool_ versions, so
those need their own sweep.

This repository is half-converted, and knowing which half you are in matters.
Thirty-one `uses:` lines carry SHAs, across six distinct actions. Thirty-five
lines across fourteen files are still tags, including
`anthropics/claude-code-action@v1`, which is the action that holds the session
credential. `actions/upload-artifact` appears at both `@v4` and `@v7` in the
same tree, which is what a half-swept tag pin looks like from the inside.
Convert the file you are touching; do not open a thirty-five-line sweep PR that
nobody can review.

## Keep the logic out of the YAML

**YAML decides when and with what. A program decides what.** A `run:` block is
code that can only be tested by pushing.

ADR-0020 already settled where that program lives: `cargo x` for anything that
produces or measures a published number, `.github/scripts/` for CI glue, as
files, never hidden in a YAML heredoc. Read it rather than re-deriving it. What
this skill adds is the test that tells you which one you are holding: if the
logic has a branch in it, it belongs in a program with a self-test.
`.github/scripts/*.test.mjs` is that self-test, and `ci.yml` runs the glob, so a
new script's tests are picked up with no wiring.

- `set -euo pipefail`.
- Never interpolate `${{ }}` into a shell body. Pass it through `env:` so a
  branch named `"; rm -rf /` is a string and not a statement. Three lines in
  this tree still do it (`ci.yml:65`, `ci.yml:72`,
  `agent-model-intel.yml:120`); all three interpolate values GitHub controls
  rather than values a contributor does, which is why they are debt and not an
  incident. Fix the one in the file you are touching.
- A `prompt:` block takes no `${{ }}` either, for an unrelated reason worth
  knowing before you reach for one. A single expression makes the **whole
  block** an expression, and an expression is capped at 21,000 characters.
  `agent-bdfl.yml`'s prompt is around 26,000, and GitHub rejected every push of
  that file on 2026-08-23 with `Exceeded max expression length 21000` until the
  last interpolation came out. Personas arrive via
  `--append-system-prompt-file`, other dynamic values via `env:`. All 28
  `${{ }}` in that file sit outside the prompt block, in `env:`, `with:` and
  `permissions:`; the rule is recorded at `agent-bdfl.yml:225`.

## Caching

Caching is not free and not always a win. The rules that outlive the numbers:

- Entries are evicted by **last access**, so a key that changes every run never
  hits and evicts everything that would have.
- **Scope runs downhill.** A branch reads its own caches, its base's, and the
  default branch's; the default branch cannot read a feature branch's. Warm the
  cache on `main` if pull requests are to hit it.
- A pull request's cache is scoped to its merge ref, so every later run of that
  pull request restores it and nothing outside the pull request can.
- The key is exact; `restore-keys` is the prefix ladder below it.

Do not cache what is cheap to fetch. A toolchain cache saving forty seconds is
a bad trade the moment it evicts a build cache saving four minutes.
[Cache limits, eviction and scope][cache].

## Minutes, and where they actually go

Billing is per job and rounded up, so many short jobs cost more than the same
work in one, and a matrix of trivial jobs is the usual way a bill triples
without anything getting faster.

A public repository is charged nothing for standard runners, which is not the
same as free. Concurrent jobs are limited **per account**, not per repository,
and this account runs twenty workflow files, seven of them
model-carrying agent seats. Optimise as though the
minutes were billed: the discipline is identical and only the unit changes.

What saves minutes, in order of effect:

1. **Do not start the run.** Path filters, and a `concurrency` group that
   cancels superseded runs. Every workflow in this tree declares one; keep it
   that way.
2. **Key that group on the event as well as the ref.** On the ref alone a
   `push` run and a `pull_request` run cancel each other, and a required check
   reading _Canceled_ is not a check that passed.
3. **Fewer, longer jobs.** Every job pays its own allocation, checkout and
   toolchain install.
4. **Split only for parallelism you will wait on**, or for a check name a rule
   needs. Not for tidiness.
5. **`timeout-minutes` on every job.** The default ceiling is hours; a hung job
   bills all of it.
6. **Order cheap before expensive.** The twenty-second linter first.

`codeql.yml` is the worked example of 1 and 3 in this repository, and its
comments carry the reasoning that a diff cannot.

## Third-party actions

An action runs with this project's secrets and may write to the repository
with the `GITHUB_TOKEN`, so [one compromised action compromises the
workflow][secure]. Decide
per action, and write the decision down:

- **Could four lines of shell do it?** Most `uses:` for "set an output" or
  "comment on a PR" is `gh api` with a supply chain attached. This repository
  has `.github/scripts/gh-comment` for exactly that reason.
- **Does it need the secret?** Split the job so the step touching the
  credential is one of ours.
- **Did you read the code at that SHA?** A pin to code nobody read is a pin to
  a stranger's future self.

Most dangerous: anything on `pull_request_target` or `workflow_run`, which are
privileged for forks too; anything wanting `id-token: write`; anything that
curls a script at runtime, which makes the pin decorative.

## The rule that costs everyone a day

[**GitHub raises no workflow run from an event that its own `GITHUB_TOKEN`
caused.**][token] A review posted by the workflow token raises an event nothing is
subscribed to, so a two-workflow loop silently does not exist.
`workflow_dispatch` and `repository_dispatch` are the exceptions.

This project has already paid for this rule twice, and
`agents/GOVERNANCE.md` "Push identity" is the single record of what each
credential can and cannot do. Read it before choosing a token; do not re-derive
it here. The two scars, so you recognise the symptom:

- PR creation cannot move to the workflow token the way issue creation did,
  because `ci` and `agent-review` trigger on `pull_request` alone. A PR opened
  on that identity would be born with no checks, permanently (issue #489).
- No `claude-code-action` seat carries a `schedule:` trigger. GitHub attributes
  a scheduled run to whoever it decides last touched the bot-edited file, and
  the OIDC token exchange rejects a bot actor, so the clock is a Cloudflare
  Worker dispatching on the operator's PAT (ADR-0035). The qualifier is the
  whole rule: a script-only workflow exchanges no token and keeps its native
  cron safely, which seven files here do, `agent-model-intel.yml:33` among
  them. Read ADR-0035 and GOVERNANCE "Push identity" before adding either
  kind.

A second exception with a different symptom: a pull request the token opens or
updates, including by pushing to an open one, **does** create a run, held in an
approval-required state. That is a different fix.

## Auditability

A run that fails and says nothing costs more than a run that never happened.

- `::notice::` / `::warning::` / `::error::` become annotations. One line
  naming the cause beats a wall of stderr.
- `$GITHUB_STEP_SUMMARY` is the readable account of a decision; it dies with
  the logs, so anything durable goes in an artifact with a deliberate
  `retention-days`.
- Artifacts are downloadable by anyone who can see the run. On a public
  repository that is everyone: **never put a credential in one.** GitHub masks
  secrets in workflow logs only (CLAUDE.md rule 10).
- Check **names** are the integration surface. A required check that never
  reports blocks every merge, so never require a name that only exists on some
  events, and never require a step's name, which is not a check.
- `always()` on the step that records, or you lose exactly the runs worth
  keeping.
- A scanner that writes somewhere no workflow reads is not coverage. If the new
  workflow produces findings, name the consumer in the same PR;
  `.github/scripts/security-alerts` is what that costs when it is retrofitted
  instead.

## Before you open the pull request

- [ ] Every `uses:` you touched is a full SHA with an accurate version comment.
- [ ] `permissions:` declared, least-privilege, raised per job, remembering
      that a job block replaces rather than extends.
- [ ] No `${{ }}` interpolated into a shell body.
- [ ] `timeout-minutes` on every job; `concurrency` declared, keyed on event
      and ref for per-branch runs.
- [ ] Logic worth being wrong about lives in a program with a self-test
      (ADR-0020).
- [ ] The check name matches what the ruleset requires, exactly.
- [ ] You can name who can trigger this and what they reach.
- [ ] If two workflows talk to each other, you have checked the `GITHUB_TOKEN`
      rule above.
- [ ] `cargo x check` passes.

One item on that list is not yet enforceable here and is named rather than
quietly dropped: **nothing lints the workflow YAML.** `actionlint` is not in
`cargo x check` and not in `ci.yml`, so every item above is checked by a
reader or not at all. A checklist whose enforcement does not exist is one the
next worker learns to skip, so it is on the record as debt, not as a rule this
repository keeps.

[billing]: https://docs.github.com/en/billing/concepts/product-billing/github-actions
[cache]: https://docs.github.com/en/actions/reference/dependency-caching-reference
[events]: https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows
[secure]: https://docs.github.com/en/actions/reference/secure-use-reference
[syntax]: https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax
[token]: https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow
