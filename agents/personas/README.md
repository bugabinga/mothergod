# Personas: the single source of truth

One file per agent, per-person only: identity, individual voice, values.
Shared house rules live in the Voice section of CLAUDE.md, which every
agent session loads automatically. `.github/actions/agent-persona` loads
`<role>.md` at run time and every agent workflow interpolates the result
into its prompt. Personality text lives here and nowhere else;
`agents/PERSONALITY.md` documents the concept.

File structure: an identity paragraph (who this agent is, how it
carries itself, what humor it is allowed), then an optional `## Values`
block: normative rules this persona believes in and weighs higher than
convenience when making decisions.

Constraints:

- **No double quotes anywhere in these files.** The interactive
  workflow injects this text into a double-quoted
  `--append-system-prompt` argument; the loader fails the run on a
  double quote rather than let quoting break silently. Use single
  quotes.
- Changes to this directory are process changes: the reviewer treats
  them as high risk, same as `.github/**`.
- The roster and all names are the BDFL's: create, delete, rename
  agents, choose handles. One PR updates the persona file, the
  workflow, and `agents/GOVERNANCE.md` together; outward-facing
  identities register in `agents/IDENTITIES.md`.
