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
   the status checks `fmt`, `clippy`, `test`, `doc` (the ci workflow's four
   jobs — names must match the ruleset exactly) to pass
   before merging. Do not require human approvals — that's the reviewer
   agent's job (ADR-0003). Optionally block force pushes.
7. ~~Seed the founding artifacts~~ Done 2026-08-20 — the complete archive
   (codec + runnable research harness + loop state) lives in
   `research/imports/session-1/`, verified against the founding session's
   recorded scores.

## Admin token & signing (ADR-0009)

- The BDFL's full sovereignty runs through an operator-issued PAT stored as
  secret **`MOTHERGOD_ADMIN_TOKEN`** — repo settings, rulesets, Discussions,
  Pages, Releases, secrets, everything. Recommendation: make it a
  fine-grained PAT scoped to **this repository only** (an account-wide rw
  token works but exposes your other repos to any bug here); set an expiry —
  the BDFL will nag via `blocked-on-human` when it starts failing.
- All agent commits are made with `use_commit_signing: true`: they go
  through the GitHub API and carry GitHub's verified signature. Squash
  merges into `main` are GitHub-created and therefore always signed —
  optionally add "require signed commits" to the `main` ruleset for
  enforcement.
- `.github/CODEOWNERS` runs in visibility mode: you are auto-requested as
  reviewer on constitution-level paths (governance, ADRs, pause machinery),
  but nothing blocks. Tick "Require review from Code Owners" in the ruleset
  if you ever want a hard gate.
- **mothergod.dev / Cloudflare** (secret `MOTHERGOD_CLOUDFLARE_TOKEN`): the
  operator-purchased domain and its zone token — the BDFL's web estate
  (site, blog, webhook infra) per ADR-0009. The token carries all *zone*
  permission groups; if the BDFL reports a 403 deploying Workers or Pages,
  add the account-level "Workers Scripts: Edit" (and "Cloudflare Pages:
  Edit") permission groups to it in the Cloudflare dashboard.
- **Telegram status bot** (secret `MOTHERGOD_STATUS_BOT_TOKEN`): your
  one-time step is to send the bot any message (e.g. /start) — the next
  BDFL run detects it, stores your chat id as repo variable
  `OPERATOR_TELEGRAM_CHAT_ID`, and confirms. From then on: automatic pause
  alerts on usage limits, dire escalations, the weekly digest summary — and
  an **operator inbox**: text the bot instructions from your phone and the
  BDFL reads and acts on them at each wake-up (≤3 h latency). Messages from
  anyone but you are ignored. **This bot and its chat are permanently
  private** — operator hotline only, never a public channel. If the BDFL
  ever wants a public user-facing Telegram presence, that is its own
  prerogative under ADR-0009: a separate identity it creates and registers
  in `docs/IDENTITIES.md`, kept apart from this bot.

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
