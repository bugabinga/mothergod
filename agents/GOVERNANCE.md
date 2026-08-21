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
- *BDFL driver* — every three hours (ADR-0007): the project director
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
| Model choice per agent role | BDFL, on the record (ADR-0012); the BDFL itself runs the strongest available model and may not lower itself below the operator's floor |
| Repo settings, rulesets, GitHub features, repo secrets, project identities/accounts | BDFL via the operator-issued admin token, recorded in `agents/IDENTITIES.md` where identities are involved (ADR-0009) |
| The Mission section of `ROADMAP.md` | Operator only; BDFL proposes amendments via `blocked-on-human` (ADR-0011) |
| Subscription-only Claude auth + pause-on-limit behavior | Mission-tier standing operator requirements: preserved by every agent, changed only by the operator (ADR-0004/0009/0011) |
| Releases | Agent-prepared, operator-triggered until further notice |
| Security, CoC, secrets, settings | Operator only |

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
