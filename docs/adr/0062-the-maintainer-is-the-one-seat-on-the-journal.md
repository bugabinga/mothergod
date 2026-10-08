# ADR-0062: The maintainer is the one seat on the journal

Status: accepted · Date: 2026-10-08 · Supersedes ADR-0049 (the journal's next slice is one lock) · Prompted by issue #682

## Context

ADR-0003 seated a researcher: one experiment per session from `research/JOURNAL.md`'s standing leads, on its own cadence.
ADR-0049 found a second seat on the same source: heartbeat duty 4c falls through to the journal's top standing lead whenever the `product` queue is empty, and after #594 built one slice twice it gave the two seats one concurrency group.
The two mandates were one mandate: the researcher's prompt and duty 4c name the same skill, `compression-experiment`, against the same journal; the "wild swing" GOVERNANCE credited the researcher with existed in no prompt (curator, #682, 2026-09-23).

The product queue is empty by construction: M3 and M5 are programs, not milestones (`ROADMAP.md`), so the fall-through is where the codec program executes.
Of the eleven journal slices merged 2026-09-24 to 2026-10-08 (S2-R30 to R34, S2-A109 to A115), ten shipped from heartbeat branches and one from `claude/research-*` (`gh pr list --state merged`, by branch prefix; #682).
The researcher's three-day claim window (`research-due`, #541) existed to bound one seat's spend; the seven-day allowance read 5% used on 2026-10-08, so the budget did not need the throttle, only the roster did.

The cost of the second seat: a workflow, a persona, a ladder, a claim-check script with its test, one interlock, one tick shared on the Telegram clock, and a cadence document that did not describe the cadence the journal received.

## Decision

The researcher seat retires.
The maintainer heartbeat is the one seat that takes slices from the journal, through duty 4c and `compression-experiment`.

- The roster edit is one PR, per GOVERNANCE "Roles": the workflow, the persona, the ladder in `agents/models.json`, the roles section, the worker's dispatch table and the alarm's watch list.
- The heartbeat's concurrency group returns to ADR-0014's per-seat shape: one seat, one group, `agent-heartbeat`.
  ADR-0049's lock had two parties and now has one; the record is superseded, not amended, because a reader who finds `journal-slice` in history reads it as retired.
- Commissioned research, the director's scout duty, is an issue labeled `research` and `product` carrying its charter.
  Duty 4c works it ahead of the journal's standing lead.
- `stalled-prs`'s raced-duplicate close (ADR-0049) stays: it depends on no seat, and two sessions of any kind can still race.

## Consequences

The written cadence and the realized cadence are one: the journal is worked at the heartbeat's cadence, as often as the product queue is empty, which the programs make the common case.
The herald's tick wakes the herald alone; all five cron expressions stay in use, so a new seat still joins an existing tick.
`/run researcher` on the Telegram bot is gone; a slice on demand is `/run maintainer`.

The journal's slices now run on the maintainer's rung, sonnet, where the researcher ran opus first.
Ten of the window's eleven slices already did.
If the retrospect reads the slices as thinner, the lever is the maintainer's ladder (ADR-0031), not a second seat.

## Rejected alternatives

The researcher owns the journal and duty 4c loses its fall-through (#682, option 2).
By the window's numbers it removes most of the codec work unless the research cadence rises to match, at which point the researcher is the heartbeat under another name.

Keep both and write the split down (#682, option 3).
The split does not exist in the prompts; writing it down means designing two mandates and keeping the interlock to hold them apart.
