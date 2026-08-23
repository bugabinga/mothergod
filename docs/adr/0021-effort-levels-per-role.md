# ADR-0021: Effort level is per-role data, alongside the ladder

Status: accepted · Date: 2026-08-23 · Extends ADR-0018, serves ADR-0012

## Context

ADR-0012 gives the BDFL model choice per role, reasoning that a
judgement-heavy role rewards a stronger model while a mechanical one may
not. Reasoning effort is the same axis at finer grain: the Claude Code
CLI exposes `--effort` with `low`, `medium`, `high`, `xhigh`, `max` and
`ultracode`, and the available levels depend on the model.

Nothing set it. Every agent ran at the session default, which is a choice
nobody made, the same criticism ADR-0012 already levels at an unpinned
model.

Operator's framing on reading ADR-0018: put effort where the ladder is,
so the BDFL can set both, and empty means default exactly as it does for
the ladder.

## Decision

`agents/models.json` restructures from two parallel maps to **one object
per role**:

```
"roles": {
  "bdfl": { "ladder": ["claude-fable-5", "claude-opus-5"], "effort": "" }
}
```

One object rather than a `ladders` map beside an `effort` map, because
two maps keyed by the same thing are a synchronization debt, and this
file exists to be a single source. The restructure is cheap now and
expensive later: ADR-0018 landed hours ago and nothing else reads it.

`agent-guard` emits `effort_flag` next to `model_flag`. Empty means the
session default, which is every role's shipped value: this decision
creates the mechanism and changes no behaviour.

**An unrecognized level is dropped with a log line, never fatal.** A typo
in a data file must not take an agent offline. This continues ADR-0018's
fail-open rule: everything unreadable resolves toward running.

Effort is the BDFL's to set, on the same terms as the model (ADR-0012),
in the same file, by an ordinary commit.

## Consequences

The BDFL gains a second, cheaper knob. Raising effort on a role that is
reasoning badly is a smaller change than moving it up a model tier, and
it can be tried and reverted in one commit.

**What the guard cannot validate.** "Available levels depend on the
model", so a level valid for one rung may be rejected by another. The
guard checks the value against the six documented names and nothing more;
a model-specific rejection surfaces at run time as an action failure.
That is thin, and the honest reason it is acceptable is that the failure
is loud, immediate, and traceable to a one-line data change.

**Interaction with the ladder is unhandled.** Effort is per role, not per
rung, so an agent falling back from Fable to Opus carries the same effort
level to a model that may price or support it differently. If that turns
out to matter, effort moves inside each ladder entry, which is why the
per-role object exists rather than a flat pair of fields.

Existing behaviour is unchanged until someone sets a value, so this ADR
is a mechanism with no live effect. That is deliberate: the levers arrive
before the judgement about how to pull them, and the judgement is the
BDFL's.
