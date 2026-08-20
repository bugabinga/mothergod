# Operations manual (for the human operator)

Everything the agent system needs from you, and how to steer it.

## One-time setup checklist

The system is inert until these are done:

1. **Install the Claude GitHub App** on this repository:
   <https://github.com/apps/claude> (or run `/install-github-app` in Claude
   Code). Grants the agents their git/PR identity (`claude[bot]`).
2. **Create the subscription token**: run `claude setup-token` locally
   (requires an active Claude subscription), copy the OAuth token.
3. **Add the secret**: repo → Settings → Secrets and variables → Actions →
   New repository secret → name `CLAUDE_CODE_OAUTH_TOKEN`, value = the token.
   Per ADR-0004 this is the *only* Anthropic credential; never add an API key.
4. **Actions settings** (Settings → Actions → General):
   - Workflow permissions: "Read and write permissions" is NOT required
     (workflows declare their own), but **enable** "Allow GitHub Actions to
     create and approve pull requests" — the reviewer's approvals and the
     interactive agent's PRs need it.
5. **Enable auto-merge** (Settings → General → Pull Requests → "Allow
   auto-merge") and **squash merging**.
6. **Branch protection / ruleset on `main`** (Settings → Branches): require
   status check `fmt + clippy + test + doc` (the `quality-gate` job) to pass
   before merging. Do not require human approvals — that's the reviewer
   agent's job (ADR-0003). Optionally block force pushes.
7. ~~Seed the founding artifacts~~ Done 2026-08-20 — the complete archive
   (codec + runnable research harness + loop state) lives in
   `research/imports/session-1/`, verified against the founding session's
   recorded scores.

## Steering

- **Give the team work**: open an issue. The heartbeat triages daily.
- **Talk to an agent**: write `@claude …` in any issue or PR.
- **Change priorities**: edit `ROADMAP.md` (directly on main if you like —
  you're the operator).
- **Change agent behavior**: edit `CLAUDE.md` or the prompts in
  `.github/workflows/agent-*.yml`.
- **Trigger a session manually**: Actions tab → agent-heartbeat /
  agent-research → Run workflow.

## Pausing and resuming (ADR-0004)

- **Automatic**: when any agent run hits a Claude usage limit, the system
  opens an issue labeled `agents-paused` with a `RESUME-AT: <ISO time>` line
  (+6 h rolling-window, +24 h weekly limit). Every agent workflow skips while
  it's open; the first scheduled run after RESUME-AT closes it and resumes.
- **Manual pause**: open an issue yourself with label `agents-paused`.
  No `RESUME-AT` line = paused until you close it.
- **Manual resume**: close the pause issue.
- CI (`ci.yml`) and Dependabot are *not* paused — they cost no Claude usage.

## Watching the system

- The **ops-log issue** (label `ops-log`) gets a short status comment after
  every heartbeat — the team's daily standup — and a weekly **BDFL digest**
  (decisions, process changes, and your personal nag list of
  `blocked-on-human` items).
- `research/progress.jsonl` + `research/JOURNAL.md` — the experiment record.
- Issues labeled `blocked-on-human` — your personal work queue; the agents
  put things there when only you can act.

## Emergency stops

- Pause (above) stops all Claude usage.
- Disable individual workflows: Actions tab → select workflow → "…" →
  Disable.
- Nuclear: remove the `CLAUDE_CODE_OAUTH_TOKEN` secret.

## Known limitations (accepted)

- The reviewer's merge (github-actions token) does not trigger `ci.yml` on
  `main`; the PR's own run validated the identical tree.
- Fork PRs are not auto-reviewed (no secrets on fork events); the daily
  heartbeat reviews them instead.
- A paused system does nothing at all — including review — until resume.
