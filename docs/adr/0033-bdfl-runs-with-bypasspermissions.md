# ADR-0033: The BDFL seat runs with bypassPermissions

Status: accepted · Date: 2026-08-27 · Prompted by issue #150

## Context

`.claude/` is a Claude Code hardcoded protected path. The safety check that
guards it runs before `permissions.allow` / `--allowedTools` evaluation, so
no allowlist entry pre-approves a write there. In a non-interactive `-p` run
there is no prompt to fall back to, so the write is denied outright. Verified
live (issue #150): both `Write`/`Edit` and `Bash` redirection into
`.claude/skills/` are denied under the BDFL's normal permission mode.

Issue #150 found the BDFL prompt carries roughly 4,700 characters of
procedure (delta-core order, Telegram protocol, shipping steps, digest
format) that ADR-0025 says belongs in a skill loaded on invocation, not
inlined into every wake. The extraction needs to write
`.claude/skills/bdfl/SKILL.md`. Every file currently under `.claude/skills/`
was authored by the operator directly; no agent PR has ever touched that
tree, because no agent seat can.

`--permission-mode bypassPermissions` is the only mechanism that clears the
protected-path check in a non-interactive session. It is not scoped to
`.claude/`: it opens the whole protected-path list for that session,
including `.git` internals, `.mcp.json`, and the session's own transcript.
The transcript is the one with a standing dependency: `retrospect` and the
audit artifact (ADR-0023) treat it as ground truth for what a session did,
which stops holding the moment the audited seat can rewrite its own record.

The BDFL already runs with unrestricted `Bash`, `Write`, `Edit`,
`MultiEdit`, and the admin PAT (ADR-0008, ADR-0011). Against that existing
envelope, bypassPermissions adds transcript and `.git`/`.mcp.json` write
access as the marginal risk; everything else in the protected-path list is
already reachable through the tools the seat holds today.

## Decision

`agent-bdfl.yml` runs with `--permission-mode bypassPermissions`. The other
four workflows (review, heartbeat, research, deslop) are unchanged and keep
their restricted permission modes; none of them author skills or otherwise
need protected-path writes.

## Consequences

The BDFL can create and edit `.claude/skills/` content directly, unblocking
ADR-0025-style extractions (starting with the `bdfl` skill itself) without
routing drafted content through the operator.

The transcript-integrity assumption behind `retrospect` and ADR-0023 no
longer holds structurally for BDFL runs; it holds only because the BDFL has
not been directed to rewrite its own transcript and has no reason to. If a
BDFL session's transcript is ever found altered outside the normal upload
path, treat that as the signal this ADR's risk acceptance was wrong, not as
routine drift.
