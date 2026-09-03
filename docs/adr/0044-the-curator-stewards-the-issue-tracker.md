# ADR-0044: The curator stewards the issue tracker

Status: accepted · Date: 2026-09-03 · Extends ADR-0003, ADR-0005 · Prompted by operator directive (Telegram, 2026-09-03)

## Context

The issue tracker is the system's work queue: every seat picks from
it, so a vague, stale, or misrouted issue taxes every downstream
session more than it cost to write. Stewarding it was duty 4 of 5 on
the maintainer heartbeat, behind red PRs, fork PRs, and dependabot,
plus a bootstrap block at the top of its prompt. A queue-position
duty loses to everything above it, and first-touch triage was all
the position afforded: no seat ever revisited an issue after its
first label, so accepted issues drift stale with nothing to notice.

The operator's directive, 2026-09-03: move bootstrap and triage out
of the maintainer so it can focus on product and code, and create a
seat that also grooms, discusses, and critiques issues — not a
labeling clerk but an adversarial sparring partner that keeps issue
quality high and current.

## Decision

One new seat, the **curator**, stewards the issue tracker; the
maintainer keeps fixing and shipping. The charter:

- **Bootstrap**: labels and the ops-log issue exist, idempotently,
  moved verbatim from the heartbeat prompt.
- **First-touch triage**: every issue leaves with a realm label and
  one of three fates — closed with the reason, `blocked-on-human`
  with the exact ask, or accepted into a queue. Triage decides; it
  does not just label.
- **Standing grooming**: revisit open issues for staleness (the
  claim no longer matches the repo), duplicates, scope braids (two
  ideas in one issue), and missing evidence. Close what shipped,
  challenge what drifted.
- **Adversarial critique**: spar on substance — falsifiability,
  evidence, scope, priority — with every author including the BDFL
  and the operator. Never on wording or style: PERSONALITY.md's
  no-policing rule bounds the seat. A critique names the defect and
  the smallest fix; a rebuttal that answers it gets a concession,
  not a re-litigation, and after two unanswered critiques on one
  issue the curator stops repeating itself.

Boundaries: no `Edit`/`Write` — the seat judges and routes, never
modifies the tree (the reviewer precedent in GOVERNANCE.md "Tool
envelopes"). It opens no PRs and never merges; machinery defects it
finds become `agent-system` issues. It applies `blocked-on-human`
but never removes it (the latch rule). Security reports stay
operator-only (ADR-0032). Cadence rides the worker clock and the
model ladder lives in `agents/models.json`, both BDFL-set like
every seat's.

## Consequences

- The maintainer's prompt shrinks to fixing and shipping; its queue
  arrives pre-routed.
- One more daily discretionary wake; the allowance governor
  (ADR-0039) throttles it like any other.
- Every curator comment is permanent project surface; the voice
  rules bind and the BDFL's retrospect judges each run.
- Issue authors now get pushback. That is the point: a critique
  ignored twice goes quiet, an accepted critique makes the issue
  cheaper for whoever executes it, and a disagreement that will not
  converge escalates to the BDFL instead of looping.

## Rejected alternatives

- **Keep triage on the maintainer with a sharper prompt.** Duty 4
  of 5 stays duty 4 of 5; the directive exists because position,
  not phrasing, was the defect.
- **Fold triage into the BDFL.** First-touch triage is daily
  mechanical routing; on the director's seat it would crowd out
  direction, and the sparring partner would be grading its own
  issues.
