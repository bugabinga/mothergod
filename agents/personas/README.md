# Personas: the single source of truth

One file per agent, per-person only: identity, individual voice, values.
Shared house rules live in the Voice section of CLAUDE.md, which every
agent session loads automatically. Every agent workflow reads its file
directly, as `--append-system-prompt-file agents/personas/<role>.md` in
`claude_args`: the persona is a system prompt, and nothing interpolates
it into a prompt block, because one `${{ }}` caps a block at 21,000
characters (issue #154). Personality text lives here and nowhere else;
`agents/PERSONALITY.md` documents the concept.

File structure: an identity paragraph (who this agent is, how it
carries itself, what humor it is allowed), then an optional `## Values`
block: normative rules this persona believes in and weighs higher than
convenience when making decisions.

Constraints:

- Changes to this directory are process changes: the reviewer treats
  them as high risk, same as `.github/**`.
- The roster and all names are the BDFL's: create, delete, rename
  agents, choose handles. One PR updates the persona file, the
  workflow, and `agents/GOVERNANCE.md` together; outward-facing
  identities register in `agents/IDENTITIES.md`.
