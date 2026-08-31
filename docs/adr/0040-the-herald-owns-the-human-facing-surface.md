# ADR-0040: The herald owns the human-facing surface

Status: accepted · Date: 2026-08-31 · Extends ADR-0005, ADR-0011 · Prompted by operator directive (Telegram, 2026-08-31)

## Context

The mission wants an open-source product real humans choose. The
BDFL charter (ADR-0005, extended by ADR-0011) parked every non-code
product aspect — site, README voice, positioning, release notes,
blog — on the BDFL seat, as a sixth priority behind unblocking,
pruning, reprioritizing, machinery, and direction. Priority six of
six never wins a run. The evidence is the site: mothergod.dev is
technically accurate and reads like an agent reporting internal
state to itself. Its status box says no benchmark-suite number
exists while the section under it claims validation on Silesia and
Canterbury; it cites journal entry ids and `FORMAT_VERSION` at
strangers. The operator's verdict, 2026-08-31: the content
"screams AI generated slop", and the duty should move to one or
more dedicated agents with carefully crafted, deliberately
less-technical context, audience-structured content, measurable
success benchmarks, and enough built-in audit to improve the seats
over time.

The USERS scorecard metric is also unmeasured, and the scorecard
rule says an unmeasurable metric outranks the work it would
measure.

## Decision

One new seat, the **herald**, owns making the human-facing surface;
the BDFL keeps steering it. One seat rather than a team, because
the obvious split (a maker and a researcher/measurer) is a braid:
the researcher's findings are exactly the maker's context, and two
seats would pay a synchronization debt from day one. The division
the work actually has is temporal, not functional, and one seat
carries it as two modes, the same shape the BDFL's delta/survey
split already proved:

- **Steward runs** (routine wakes): fix its own red or
  changes-requested PRs first, then ship one reader-facing
  improvement per run, from the `marketing`-labeled issue queue or
  found on the surface itself.
- **Weekly survey** (self-healing, first wake of the UTC week
  without a survey entry): study one successful OSS project's
  marketing, measure the audience numbers (stars, forks, external
  authors, site analytics, mentions queried read-only), and record
  both in the journal. This makes USERS measurable; the BDFL's
  weekly digest reads the numbers from there.

Charter boundaries:

- Territory: `site/`, `README.md`'s reader experience,
  release-notes and `CHANGELOG.md` voice, positioning,
  `marketing/`, and any owned channel in `agents/IDENTITIES.md`.
  Never `src/`, never the format spec's content, never another
  agent's prose (PERSONALITY.md's no-policing rule).
- Institutional memory: `marketing/JOURNAL.md`, the audience
  model, measured numbers, learnings, and rejected approaches, in
  the classical realm because it is product work a human
  contributor can read and audit. Same role as
  `research/JOURNAL.md` for the researcher.
- Honesty binds hardest here: every public claim carries its
  evidence, external platforms stay read-only instruments, no
  astroturfing ever (mission non-negotiable). The herald enforces
  truth on the outward surface.
- Ships by the normal pipeline: opens PRs, never merges; the
  reviewer approves. Model ladder and cadence live in
  `agents/models.json` and the worker clock
  (`infra/telegram-worker/wrangler.toml`), both BDFL-set like every
  seat's.

Audit and improvement are built in, not promised: the seat uploads
the standard audit artifact (ADR-0023), the BDFL's retrospect
judges every run, the journal makes its reasoning public, and the
weekly digest carries its numbers. The split trigger is named now
so future division is evidence-based: when survey duties crowd
steward work out of two consecutive weeks, or the journal shows
the maker grading its own success metrics, split the measuring
half into its own seat by superseding this ADR.

## Consequences

- The BDFL's product-surface duty becomes steering: charter issues,
  retrospect judgment, prompt evolution. Its prompt and
  `agents/GOVERNANCE.md` change in this PR.
- The heartbeat's product queue excludes `marketing`-labeled
  issues, one more routing label to keep true.
- A new twice-daily wake spends allowance; the governor (ADR-0039)
  throttles it like any discretionary wake.
- The site gets a full-time owner, and the operator gets a seat to
  blame, which is what "full responsibility" means.

## Rejected alternatives

- **A marketing team (maker + researcher + strategist).** Three
  personas to tune blind, triple wake cost, and every handoff a
  document nobody else reads. Rejected until the named split
  trigger fires; the operator asked for a structure that can grow,
  not one that starts grown.
- **Keep the duty on the BDFL with a bigger prompt.** Priority six
  of six stays priority six; the evidence of two weeks says it
  starves.
