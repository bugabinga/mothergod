# Trusted sources

The BDFL's reading list for staying current on the craft this project runs
on: Claude models and features, token efficiency, context engineering,
agentic-system and skills best practices — whatever becomes the new smart
way to build software factories. Reviewed on the weekly deep run.

Rules: operator-seeded, BDFL-curated — the BDFL may add/remove sources on
the record (say why in the digest). Adopting a practice from a source is a
machinery change like any other: state what changed and what evidence
would show it helped (FLOW/HEALTH). Never adopt something solely because
it is new; never skip something solely because it is work.

## Operator's trusted sources

<!-- Seeded by the operator 2026-08-20; add freely. -->

| Source | Why |
|---|---|
| Theo Browne — https://t3.gg, https://www.youtube.com/@t3dotgg | t3/T3 stack; fast, opinionated coverage of AI-tooling and model shifts as they land |
| Mario Zechner — https://mariozechner.at, https://github.com/badlogic | author of the pi coding agent (which the operator uses); deep hands-on agent-harness engineering, minimal-and-honest tooling philosophy |
| Dex Horthy — https://humanlayer.dev, https://github.com/humanlayer/12-factor-agents | HumanLayer; 12-Factor Agents and advanced context engineering — reference thinking for production agent systems |

## Defaults (assistant-seeded 2026-08-20; BDFL may prune)

| Source | Why |
|---|---|
| https://www.anthropic.com/engineering | Anthropic's engineering posts — context engineering, agent patterns, evals |
| https://code.claude.com/docs | Claude Code docs — features, hooks, skills, GitHub Actions usage |
| https://github.com/anthropics/claude-code/blob/main/CHANGELOG.md | Claude Code CLI changes that affect every agent session |
| https://github.com/anthropics/claude-code-action/releases | The action all workflows run on — inputs, fixes, behavior changes |
| https://docs.claude.com/en/release-notes/overview | Claude API/model release notes — new models, pricing, efficiency levers |
| https://www.anthropic.com/news | Model announcements (which model tier should each agent run?) |
| https://developers.cloudflare.com/agent-setup/prompt.md | Cloudflare's official agent bootstrap — MCP servers + skills for the web estate |
| https://developers.cloudflare.com/agents/ | Cloudflare's agents documentation |
| https://simonwillison.net | High-signal independent commentary on LLM/agent engineering practice |

## Adoption log

Newest first. One line each: date, source, what was adopted or rejected, why.

- (empty)
