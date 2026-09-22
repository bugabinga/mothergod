# ADR-0049: The journal's next slice is one lock

Status: accepted · Date: 2026-09-22 · Amends ADR-0014 (two seats share one concurrency group) · Prompted by issue #594

## Context

ADR-0014 gave every scheduled seat its own concurrency group, so that no seat queues behind another and an operator wake starts within seconds.
Its consequences named the condition for revisiting it: a real conflict class produced by concurrent sessions.

Two seats take work from one source.
The researcher runs one experiment from `research/JOURNAL.md`'s standing leads.
The maintainer, when the `product` queue is empty, works the journal's top standing lead (heartbeat duty 4c).
Both read the same journal and reach the same next slice by construction.
Each runs a claim check at session start that reads open PRs and `claude/*` branches, and each publishes its own claim only at session end, when it opens its PR.
Two sessions in flight together therefore see no claim, and the race window is the length of a session.

On 2026-09-18 a research run and a heartbeat started a minute apart and both built `build_encode_table`: PR #591 merged, PR #592 conflicted at birth and surfaced as a `never-fired` stall, and one full session was discarded (issue #594).
The fall-through is not incidental.
Of the codec research PRs merged between 2026-09-18 and 2026-09-22, the maintainer's branches carried 21 and the researcher's 2 (`gh pr list --state merged`, by branch prefix).

## Decision

The two seats that take slices from the journal share one concurrency group, `journal-slice`, with `cancel-in-progress: false`.
A session in either seat starts only after the other seat's session has ended, and a session ends with its PR open or its branch pushed, so the claim check each seat already runs at start sees the other's slice as taken.
No claim mechanism is added.

ADR-0014's rule is restated rather than reversed: a concurrency group guards a contended write surface.
For every other seat that surface is the seat itself, and those groups stand.
The journal's next slice is the one surface two seats write.

`stalled-prs` tells the duplicate apart from the stall it resembles: a `never-fired` head whose change `main` already carries closes as raced, with its branch preserved, instead of receiving the mechanical merge of `main` the signature otherwise prescribes.

## Consequences

A research tick that lands during a heartbeat waits for it, and the reverse; a heartbeat session typically runs under twenty minutes.
GitHub keeps one running and one pending run per group, so a third arrival evicts the older pending run.
A research tick evicted that way returns at the next tick, because `research-due` answers `due` for anything it cannot decide (#541).
Operator wakes are untouched: the BDFL keeps its own group.

The two seats stay on the journal.
Whether both should be there is a cadence decision, not a bug fix: the written research cadence is about two sessions a week, the realized one is the heartbeat's eight wakes a day, and issue #682 owns that question.

## Rejected alternatives

A claim posted at pick time and re-checked before publish (#594 items 1 and 2).
It adds a script, a claim a model matches to a slice by eye, and a residual window the size of the pick; the lock adds no code and leaves no window.

Deleting the maintainer's fall-through.
It closes the race by removing a seat from the journal and, by the week's numbers, removes most of the codec work with it; a change of that size is #682's to make deliberately.

One issue per slice inside a milestone (#594 item 4).
M3 is a program without a finish line, so it has no GitHub Milestone (ADR-0047), and its slices are named by the journal as the work reveals them.
