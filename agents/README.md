# agents/

Everything about the agent system that develops and runs this project lives
here — kept strictly apart from the classical software project (ADR-0010).

| File | What |
|---|---|
| `GOVERNANCE.md` | roles, decision rules, the operator's veto |
| `OPERATIONS.md` | the human operator's manual: setup, secrets, steering, pause |
| `PERSONALITY.md` | house temperament + per-agent personas |
| `SOURCES.md` | trusted reading list + adoption log (stay-current duty) |
| `IDENTITIES.md` | registry of accounts/domains the project owns |

Two agent-system pieces cannot live here, by platform requirement:

- **`/.github/workflows/` and `/.github/actions/`** — the executable agent
  processes (heartbeat, reviewer, researcher, BDFL, interactive, pause
  machinery). GitHub only runs workflows from `.github/`.
- **`/CLAUDE.md`** — the agent contract. The Claude Code harness loads it
  from the repository root.

Everything else in the repository — `src/`, `docs/`, `research/`,
`assets/`, the community files at root — is the classical open-source
project the agents work *on*. The one shared artifact: `docs/adr/` is a
single decision series covering both realms, because a project keeps one
history of its decisions, not two.

Placement rule for new files: if it configures, describes, or steers an
agent, it goes here (or `.github/` if it must execute). If a human
contributor to the *compressor* would need it, it goes in the classical
tree.
