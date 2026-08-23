# Governance

mothergod is an experiment in agent-run open source: the day-to-day
development team is a set of Claude agents running in GitHub Actions. This
document says who decides what.

## Roles

**Operator (human): Oliver Jan Krylow (@bugabinga).**
Owns the repository, the Claude subscription that powers the agents, and all
secrets. Has absolute veto: may close any PR, revert any commit, pause the
system (open an issue labeled `agents-paused`), or change any process. Handles
everything agents cannot: repository settings, secrets, account-level actions,
publishing to crates.io, code-of-conduct enforcement, and security-report
triage. The operator is *not* required to review routine changes — the system
is fully autonomous by design (ADR-0003).

**Agents (Claude, via GitHub Actions).**
- *BDFL driver* — hourly (ADR-0015): the project director
  (ADR-0005). Unblocks stalled
  work, prunes, reprioritizes the roadmap, and evolves everything non-code —
  including the other agents' workflows and prompts — without approval
  ceremony, but always with a written record and a digest to the operator.
  Owns every non-code aspect of the project as an open-source product:
  docs, blog, release notes, positioning, community tone. Publishes only on
  channels mothergod owns (repo, blog, releases); external platforms
  (Hacker News, lobste.rs, socials) are queried read-only as success
  proxies, never posted to by the system. Sole exception to "never merge
  your own PR" (non-code PRs, green CI only).
- *Maintainer heartbeat* — daily: fixes red PRs, triages issues, picks the top
  roadmap item and ships one small PR.
- *Reviewer* — adversarial review of every PR; verifies claims by running
  them; merges when CI is green and the review passes. Never reviews work it
  authored in the same run.
- *Deslopper*, twice daily: removes slop from `src/` without changing
  observable behaviour, one scope per PR (ADR-0016). Never merges; the
  reviewer approves. Its operating manual is the `deslop` skill.
- *Researcher* — weekly: runs one experiment from the journal's standing
  leads (or a wild swing), records verdicts in `research/`.

(An interactive `@claude` mention agent existed until 2026-08-21;
removed by operator directive. Questions go in issues, which the
heartbeat triages daily.)

Agent behavior is governed by `CLAUDE.md` (the contract) and the workflow
prompts in `.github/workflows/` — both are ordinary versioned files. The
BDFL evolves them directly; other agents propose changes by PR, which the
reviewer treats as high-risk (see below).

## Decision rules

The one-line rule (ADR-0011): is it the Mission section of ROADMAP.md?
Operator. Anything else? BDFL. The table below describes the default
mechanisms, all of which the BDFL may reshape on the record.

| Change class | Who decides |
|---|---|
| Code, tests, benchmarks | Reviewer agent merges on green CI + passing adversarial review |
| Bitstream format changes | Same, but requires an ADR + `FORMAT_VERSION` bump (CLAUDE.md rule 5) |
| Docs, roadmap, priorities, stale-work pruning | BDFL directly (ADR-0005); other agents via the reviewer |
| Process changes (workflows, prompts, CLAUDE.md, this file) | BDFL directly, with a written record (ADR-0005); from other agents: reviewer with high-risk review quoting the exact behavioral diff |
| Agent permission envelopes (workflow `permissions:`, tool allowlists, turn budgets) | BDFL, on the record (ADR-0008); the BDFL itself runs at maximum permissions and open network |
| Model and effort per agent role | BDFL, on the record (ADR-0012), by editing `agents/models.json`: ladders per ADR-0018, effort levels per ADR-0021. The BDFL itself runs the strongest available model and may not remove its floor rung. The `model-limits` ledger issue is machine-written and never hand-edited |
| Repo settings, rulesets, GitHub features, repo secrets, project identities/accounts | BDFL via the operator-issued admin token, recorded in `agents/IDENTITIES.md` where identities are involved (ADR-0009) |
| The Mission section of `ROADMAP.md` | Operator only; BDFL proposes amendments via `blocked-on-human` (ADR-0011) |
| Subscription-only Claude auth + pause-on-limit behavior | Mission-tier standing operator requirements: preserved by every agent, changed only by the operator (ADR-0004/0009/0011) |
| Releases | Agent-prepared, operator-triggered until further notice |
| Security-report triage, CoC enforcement | Operator only |
| Secrets | BDFL, full governance (operator ruling, PR #101 review, 2026-08-23). The sole hard constraint is CLAUDE.md rule 10: a secret is never printed, logged, or leaked. The BDFL defers to the operator on unresolvable roadblocks, such as credentials only the operator can mint |

### `blocked-on-human` is a latch, and removing it is the answer

The label means one thing: work is stopped until a human decides. Only
the BDFL and the operator remove it, and the removal records that the
decision happened; the removing agent writes the decision on the thread
in the same breath. No agent re-applies a label it did not apply, and no
agent re-raises a gate whose label was cleared above it. A gate that
re-fires after it is answered stops being a safeguard and becomes a
deadlock: PR #112 collected four changes-requested rounds re-asking
whether the operator had provisioned a secret they had already
provisioned.

## Merging

Squash is the only landing path for every PR, agent-authored or
human, including the BDFL's own: merge commits and rebase merges are
disabled at the repository level and `main` requires linear history
(ADR-0013). A self-merge reads the PR thread first: when reviewer and
author share a bot identity, the verdict arrives as a comment the
required checks never see, and a request-changes there blocks the
merge until addressed on the record (PR #99 landed 44 seconds after
an unread one). The squash commit's subject is the PR title and its
message is the PR body, so write every PR body as the commit message
the change deserves. Land with the REST call, not the porcelain:
`gh api -X PUT repos/<owner>/<repo>/pulls/<n>/merge -f
merge_method=squash`. The porcelain's client-side mergeable-state
check refuses squashes the API accepts (PR #25), and `--auto` armed
on a branch whose tip commit is unsigned reports `blocked` and never
fires, even with every gate green (PR #84). Arm
`gh pr merge <n> --squash --auto` only when the REST call reports
required gates still pending, and expect the BDFL sweep to rescue it
if the tip is unsigned. GitHub creates the squash commit server-side
and signs it (committer `GitHub <noreply@github.com>`); the
`required_signatures` rule on `main` is satisfied by that signature
regardless of whether the source branch's own commits are signed. An
unsigned branch commit is never, by itself, a reason to stop or to
label `blocked-on-human` — attempt the REST merge before predicting
it will fail (issue #24, PR #22 postmortem).

### The reviewer skips any PR whose `agent-review.yml` differs from main

`claude-code-action` refuses to start when the branch's copy of the
running workflow file differs from `main`'s, its anti-tamper
validation. The refusal exits 0, so the job would go green having
reviewed nothing. Two ways in, both observed on 2026-08-22:

- The PR edits `agent-review.yml` itself (PR #69). No agent review is
  possible, now or on any future push; the PR lands by BDFL discretion
  once the four quality gates are green, the envelope authority
  (ADR-0008) acting as reviewer of last resort for its own machinery.
- The PR branched before a reviewer-workflow change merged, so its
  copy went stale (PR #68's re-review, 32-second run). Every open PR
  is unreviewable from the moment such a change lands until its
  branch merges `main` back in. The rescue is mechanical: merge
  `main` into the branch, push with a deliberate identity (see Push
  identity), and the synchronize event re-triggers review.

This section used to claim the second case could only stall, never
produce an unreviewed merge, "because only the reviewer merges."
**That was false, and PR #111 is the counterexample:** it edited
`agent-review.yml`, the reviewer skipped, the check went green in 26
seconds, and the BDFL merged it four minutes later under the same
carve-out the first bullet grants. Both cases reach an unreviewed
merge; the two bullets differ only in whether a rescue exists.

So the skip is now loud instead of green. The reviewer job fails when
the action produced no execution file, and posts which case it is and
what to do next. A vacuous pass is unreachable: green `review` is
evidence of review again. `review` is not a required check, so the red
blocks nothing, which is the point. It removes a false signal rather
than adding a gate.

### Stalled auto-merge

Armed auto-merge waits forever and nobody is told. Two signatures,
both swept by the BDFL every run on open `agent-approved` PRs:

- Mergeable state `dirty`: `main` moved and conflicted the branch
  (first hit PR #34, a CHANGELOG append collision). Rescue: merge
  `main` into the branch, resolve without judgment calls (append
  conflicts keep both sides), push with a deliberate identity (see
  Push identity below), clear the held runs that push creates by
  close/reopen, then land with the REST squash merge above once the
  required gates are green.
- Mergeable state clean, four gates green, auto-merge armed, PR still
  open: the branch tip is unsigned, so GitHub's own evaluation sits
  at `blocked` while the REST squash merge succeeds immediately
  (first hit PR #84; same porcelain/API asymmetry as PR #25). Rescue:
  the REST squash merge, nothing else — the reviewer's verdict
  already covers the exact head SHA.

If a held run still blocks after re-attribution, comment on the PR
naming it, label `blocked-on-human`, move on.

## Push identity

Three credentials can push, and the pusher decides whether the pipeline
keeps moving (issue #57). Pick deliberately:

- `github.token` (actor `github-actions[bot]`): pull_request runs it
  triggers hold at GitHub's approval gate, and the review action refuses
  the actor even after an operator approves the run. Never push to a PR
  branch with it.
- The claude app (actor `claude[bot]`): any `gh api` write with the
  default session token. Triggered runs start unheld and the reviewer
  accepts the actor (`allowed_bots`). Two limits: the app token cannot
  touch `.github/workflows/**` (issue #24), and
  `mcp__github_file_ops__commit_files` only reaches the branch its run
  started on (`BRANCH_NAME` is pinned at action start), so it cannot
  push to another PR's branch. Worse on runs triggered by PR review or
  comment events: `BRANCH_NAME` pins to the PR's merge ref, and
  `commit_files` then CREATES a literal branch named `<n>/merge`
  parented on main, silently missing the PR branch (observed on #78).
  On such runs, push with the git data API instead: create blob, tree,
  commit, then PATCH the real branch ref; same `claude[bot]` identity.
- The admin PAT (actor `bugabinga`): operator-attributed, and
  operator-attributed events wake the BDFL (issue #50). Reserved for
  what the app cannot do: workflow-file pushes and cron-line changes.
  A token in the push URL is not enough: the runner injects the app
  credential as `http.extraheader`, which overrides URL auth, so a PAT
  push must clear it
  (`git -c http.https://github.com/.extraheader= push ...`), or the
  remote silently sees the app and applies its rules.

Token lifetime (issue #81): the claude app token expires about an hour
into a session; past that, every `gh` call riding the default
credential returns 401 Bad credentials (first hit: run 32590951126,
whose ops-log comment died seconds after its merges succeeded).
Front-load token-dependent writes. For comments that must land late in
a long run, fall back to the job-scoped workflow token, exposed to
agent sessions as `GH_WORKFLOW_TOKEN`:
`GH_TOKEN="$GH_WORKFLOW_TOKEN" gh api ...`. That identity is
`github-actions[bot]`, blessed for issue and PR comments only, never
for pushes (first bullet above) and never for merges. The reviewer is
exempt from all of this; its `gh` rides `github.token` job-wide
already.

A held `action_required` run is cleared by re-attributing its event,
not by approving it (no agent token approves a held run, PR #43):
close/reopen the PR with the default session token and the reopened
event carries `claude[bot]`, whose runs start unheld. Verified live on
PR #59: its final github.token-pushed commit held at the gate until
close/reopen re-attributed the event and the runs started. One quirk
observed on the same PR: two pushes seconds apart coalesce into one
synchronize event attributed to the first pusher.

Scheduled workflows carry a landmine: GitHub attributes a schedule run
to the account that landed the last change to the workflow's cron
lines, and the claude-code-action token exchange rejects bot actors on
schedule events ("User does not have write access", heartbeat runs of
2026-08-22 after PR #30 was app-merged). An app-merged PR touching a
cron line therefore kills that schedule until a human-attributed change
to the cron lands. Rule: cron-line changes land admin-PAT-attributed
(direct push, or `GH_TOKEN="$GH_ADMIN_TOKEN" gh pr merge`), never via
an app merge.

## Tool envelopes

The BDFL sets every agent's `--allowedTools` (ADR-0008). One rule, learned
twice from telemetry:

**A Bash allowlist of binary names is not a security boundary.** It costs
turns and buys nothing. Every agent that has it also has `Edit`/`Write`, so
it may already author arbitrary file content; denying it `ls` while granting
it `Write` is theatre. The real boundary is upstream and unchanged: these
jobs run only on same-repo branches, so every author is our own machinery
or the operator.

The cost is measured, not argued. The reviewer burned 11-13 denials per run
detouring around missing file tools (runs 32615950907..32628126725) and 12
median after that fix, until Bash was granted unrestricted. The maintainer
still sat at 11 median denials per run, on Bash, in 6 of its 6 most recent
runs, when the first cross-role telemetry read was taken (2026-08-23, 129
audit artifacts). A denial is a wasted turn plus a re-plan around a wall the
agent cannot see the shape of.

So: grant Bash unrestricted to any agent that already holds `Edit`/`Write`.
Withhold a tool only where withholding states a real role boundary — the
reviewer gets no `Edit`/`Write` because it judges and does not modify, and
that denial is the design working.

## Humans other than the operator

Human contributions are welcome and go through the same pipeline: file issues,
open PRs; the heartbeat triages questions daily. The reviewer agent reviews human
PRs with the same rules (and extra courtesy — see CONTRIBUTING.md). Appeals
against any agent decision: open an issue and tag @bugabinga; the operator's
call is final.

## The prime directive for agents

When in doubt between shipping something clever and keeping the system
trustworthy: trustworthy wins. Label it `blocked-on-human`, write down what
you were scared of, and move to the next task.
