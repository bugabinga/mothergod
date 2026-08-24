# ADR-0012: BDFL runs the strongest model and owns model choice

Status: superseded by ADR-0031 · Date: 2026-08-21 · Extends ADR-0008

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
