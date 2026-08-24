# agents/

Everything about the agent system that develops and runs this project lives
here — kept strictly apart from the classical software project (ADR-0010).

| File | What |
|---|---|
| `GOVERNANCE.md` | roles, decision rules, the operator's veto |
| `OPERATIONS.md` | the human operator's manual: setup, secrets, steering, pause |
| `PERSONALITY.md` | temperament concept doc |
| `personas/` | per-agent personas (identity, voice, values): single source, loaded into prompts at run time; shared voice rules live in CLAUDE.md |
| `SOURCES.md` | trusted reading list + adoption log (stay-current duty) |
| `models.json` | per-role model ladder and effort level; first available rung wins, last rung is the floor (ADR-0018, ADR-0021) |
| `IDENTITIES.md` | registry of accounts/domains the project owns |

Two agent-system pieces cannot live here, by platform requirement:

- **`/.github/workflows/` and `/.github/actions/`** — the executable agent
  processes (heartbeat, reviewer, researcher, BDFL, deslopper, pause
  machinery), plus `agent-model-intel`, which is a plain script rather
  than an agent (ADR-0019) and reports model capability alongside our own
  run economics (ADR-0023). GitHub only runs workflows from `.github/`.
- **`/CLAUDE.md`** — the agent contract. The Claude Code harness loads it
  from the repository root.
- **`/.claude/skills/`**: skills.
  `deslop` is the deslopper's operating manual (ADR-0016).
  `rust-craft` is the Rust standard for codec code, consulted by whoever is writing or judging it (ADR-0017).
  `compile-judgement` decides whether recurring work should become a mechanism (ADR-0022).
  `adr` governs creating, superseding, correcting, and reviewing architecture decisions (ADR-0030).
  The Claude Code harness only discovers skills there.

Everything else in the repository — `src/`, `docs/`, `research/`,
`assets/`, the community files at root — is the classical open-source
project the agents work *on*. The one shared artifact: `docs/adr/` is a
single decision series covering both realms, because a project keeps one
history of its decisions, not two.

Placement rule for new files: if it configures, describes, or steers an
agent, it goes here (or `.github/` if it must execute). If a human
contributor to the *compressor* would need it, it goes in the classical
tree.
