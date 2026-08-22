# ADR-0016: Deslopper agent and the `.claude/` platform exception

Status: accepted · Date: 2026-08-22 · Amends ADR-0010 (exceptions only)

## Context

Slop accumulates faster than anyone removes it. Every other seat on the
roster has a reason not to remove it: the heartbeat ships roadmap
slices, the researcher runs experiments, the reviewer holds no write
tools by design, the BDFL owns everything except code. Cleanup is
nobody's duty, so it does not happen.

The operator maintains a taxonomy of slop signals, developed outside
this project and imported here: accidental complexity, duplication,
single-use indirection, long functions, deep nesting, magic values,
error handling too thin or too thick, legacy overengineering, imprecise
types, comments that restate code. It is the operator's own standard for
what code should cost to read, and it is worth more as a standing
process than as a document nobody opens.

## Decision

A fifth agent seat: the **deslopper**. Twice daily (`37 */12 * * *`)
plus dispatch. Territory `src/` only. It removes slop and changes no
observable behaviour. It never merges: it opens PRs and the reviewer
decides, exactly like the heartbeat.

The taxonomy ships as a Claude Code skill at
`.claude/skills/deslop/`: `SKILL.md` for the scope rule and the
procedure, `references/` for the eleven signals. The workflow reads
`SKILL.md` by path rather than invoking `/deslop`, so a run never
depends on slash-command parsing inside the action.
`disable-model-invocation: true` keeps the skill out of every other
agent's listing; it is the deslopper's manual, not ambient context.

Scope is the load-bearing rule, because the failure modes on both sides
are real: single-line PRs waste review cycles, whole-repo refactors are
unreviewable. One PR is one scope. A scope is either a **place** (one
region, every defect inside it) or a **seam** (one cross-cutting
concern, every site it touches), never both. Scope inversely with blast
radius: a function with many callers, a hot path, or an invariant gets
scoped alone; a low-coupling module with no invariant of its own gets
scoped whole. File count is explicitly not the measure; a seam followed
end to end may touch twenty files and is still one scope, and splitting
it leaves the codebase speaking two idioms at once.

Behaviour preservation is proved, not asserted. Where existing tests do
not prove it, the deslopper adds the test that does, before the change,
and shows it passing on both sides.

`/.claude/` joins `/.github/` and `/CLAUDE.md` as a platform-forced
exception to ADR-0010: it is agent-realm content whose location the
harness fixes. The two-realm boundary is otherwise unchanged.

Parts of the imported material were deliberately left out where they
contradict this project. Crash-on-invalid-state contradicts hard rule 2
(the decoder never panics on any input). "No comments needed"
contradicts CLAUDE.md's write-invariants-down rule, which exists because
the session-1 port bug lived in an invariant that was never written
down. Rules about dependencies and frameworks are already ADR-0002 and
ADR-0006. Gradual-typing guidance does not apply to Rust.

## Consequences

Five agent seats instead of four. Deslopper PRs are carved out of
heartbeat duty 1: the deslopper fixes its own returned PRs, so the
heartbeat does not pick them up and the two agents do not collide on a
branch.

The reviewer's load grows by up to two PRs a day, all of them
behaviour-preserving and therefore cheap to refute: run the gates, read
the diff for behaviour change, done.

The deslopper-to-reviewer loop only closes once the fix-push route is
unblocked (issue #58). Until then a returned PR may stall at the
approval gate like any other agent PR.

Risk accepted: an agent whose whole purpose is changing working code.
The mitigations are the hard limits (never change behaviour, never edit
a test to pass, gates green on both sides), the reviewer as an
independent approver, and the narrow territory. The failure mode to
watch for is churn: rewrites that trade one idiom for another without
reducing cost. If the reviewer starts returning PRs for that, the seat
gets tightened or removed.
