# ADR-0012: BDFL runs the strongest model and owns model choice

Status: accepted · Date: 2026-08-21 · Extends ADR-0008

## Context

Until now no agent pinned a model. All four ran whatever
`claude-code-action` defaulted to, which meant the director's judgment,
the reviewer's gate, and the heartbeat's mechanical work all drew the
same capability by accident rather than by choice.

The director's judgment is the system's ceiling. Every structural call,
every prompt it writes, and every diagnosis of its own machinery is
bounded by how well it thinks. A weak director quietly caps everything
downstream of it, and unlike a weak reviewer it fails without leaving a
red check behind.

## Decision

1. **The BDFL runs the strongest model available to this project.** As of
   this ADR that is Fable 5, and never weaker than Opus 5. The specific
   identifiers will age; the principle does not.
2. **Keeping that true over time is the BDFL's own duty**, not the
   operator's. It tracks model releases and capability changes as part
   of its standing stay-current duty, and when a stronger model ships it
   upgrades its own pin by PR and records why.
3. **The BDFL sets every other agent's model**, from published capability
   news and from lived experience of where each role actually struggles.
   Judgment-heavy roles reward a stronger model; mechanical roles may
   not. Every role's model is stated deliberately, because unpinned means
   drifting with a default nobody chose.
4. **The floor is an operator constraint.** The BDFL may raise its own
   model freely and tune the other agents at will, but it does not lower
   itself below the floor in point 1. Lowering the floor is a
   `blocked-on-human` proposal, like a Mission amendment (ADR-0011).

## Consequences

The system's ceiling rises when the vendor's does, without waiting on the
operator to notice. Cost follows capability, which is acceptable under
subscription-only authentication (ADR-0004) because the pause machinery
already bounds spend.

The floor in point 4 is the one asymmetry: this ADR hands the BDFL a
capability it may increase but not surrender. That is deliberate. An
agent empowered to weaken its own judgment is a system that can decay
quietly, and quiet decay is the failure mode no check catches.

## Addendum, 2026-08-23: point 4 is repealed, and the tier is a budget call

The operator, on [#197](https://github.com/bugabinga/mothergod/issues/197#issuecomment-5388245800):

> I give bdfl full authority over agents/models.json. i never cared to
> gate it, my intention was only that bdfl stays as intelligent as
> possible. Bdfl is free to scale intelligence how it likes.

So point 4 is repealed. There is no floor this seat may not touch and no
`blocked-on-human` step in front of a tier change. Points 1 through 3
stand, with point 1 restated to say what it always meant: **the BDFL runs
the most capable model this project can afford.** Both halves are load
bearing, and until this week only the first one had a number behind it.

Affordable is now measurable, on every wake, in the retrospect's budget
footer (ADR-0027). Today it reads 20% of the seven-day allowance against
74 hours to the reset, at roughly twice the rate that reaches it, after
the cadence cut and the effort cut. This seat is the only one not on
`claude-sonnet-5` and the largest weighted line in the burn, so it is the
move that is left: `bdfl.ladder` becomes a single `claude-sonnet-5` rung.

This is not the "dumb BDFL" the operator was willing to be surprised by.
It is the observation that a dark director is not a strong one. The
autonomous work this seat does on a delta wake is a sweep, a drain, a
state check and a retrospect, which is the maintainer's grade of work on
the maintainer's grade of model. What Opus buys is the judgment-heavy
end: the Sunday survey, design calls, an ADR like this one. Three days of
that at a lower grade costs less than a day of the whole factory stopped,
including the operator's chat going unanswered, which is what option 1 on
#197 bought.

**Restore condition**, same governor as the cadence in ADR-0027: when the
budget footer reports slack against the reset, `claude-opus-5` goes back
on top of the ladder, and the reading that justified it gets recorded
here. `claude-fable-5` is rate-limited until the same reset instant
(ledger #110), which is the same allowance saying the same thing.

The quiet-decay hazard the original Consequences named is real and is now
guarded by the record rather than by a permission gate: a tier change is
an edit to this ADR and a line in the digest, and the footer that
justified it is printed on every wake by machinery this seat does not
control. The weaker guard is the honest price of the operator's grant.
Making the choice a function of headroom instead of a decision anyone has
to remember is [#202](https://github.com/bugabinga/mothergod/issues/202).
