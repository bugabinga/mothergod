# ADR-0047: Milestone state lives in the tracker

Status: accepted · Date: 2026-09-19 · Prompted by operator reports, 2026-09-18 and 2026-09-19

## Context

`ROADMAP.md` carries two kinds of content at once.
One is judgement with no machine source: what separates a milestone from a program, why M7 is placed ahead of M5, what shape the product takes.
The other is state: a checkbox per deliverable item, most of them naming the issue that delivers it.

Checkbox state duplicates issue state, and the duplicate drifts.
On 2026-09-19 a cross-check of every unticked box against its issue found #456 unticked sixteen days after it closed as completed, and #449 two days after (both fixed in PR #609).
Three earlier commits existed only to tick boxes after the fact: #473, #496, #499.

`/status` renders those boxes as milestone status, so the drift is published.
The page called M3 `pending` on a day its own ledger held 93 experiments against it (fixed in PR #601), and its milestone caption claimed a chronological order for a list ordered by priority (fixed in PR #609).
Both fixes corrected a renderer that was faithfully showing a stale source.

The operator's report of 2026-09-19 names the cause rather than the instances: a status surface nobody can judge is one whose source needs hand maintenance to stay true.

## Decision

Deliverable state lives in the issue tracker and nowhere else.

Every item of a milestone is an issue, and every milestone that is a deliverable is a GitHub Milestone of the same name holding those issues.
An item is done when its issue is closed.
`ROADMAP.md` carries no checkboxes.

`ROADMAP.md` keeps exactly what has no machine source: the mission, the milestone-versus-program distinction, the rank, the rationale for each placement, and the product shape.
Rank is the order the milestone sections appear in the file.

`.github/scripts/status-data.py` reads item state from the tracker at deploy time and keeps taking order from `ROADMAP.md`'s section order.
A read it cannot perform fails generation, as every other field of that script already does; the page never publishes a state it could not confirm.

A program has no GitHub Milestone.
It has no finish line, so it has nothing to count, which is the same line ADR-0040's status work drew between a deliverable and a program.

## Consequences

Ticking a box stops being an act anyone can forget, because closing the issue is the only act.
The drift watcher proposed in #610 is unnecessary and that issue closes: a structure that no longer exists needs no guard.

Rank stays hand-maintained prose, deliberately.
No tracker field holds it, it is editorial by nature, and a rank a reader disagrees with is visible as an argument rather than silent as a stale bit.

Deploy gains an authenticated tracker read and the network dependency that comes with it.
The page's failure mode moves from quietly stale to loudly absent, which is the trade this project already makes everywhere else.

The migration is #611; until it landed on 2026-09-21 the checkboxes were still the live source and this record described a target rather than the present.

Historical checklists for delivered milestones (M0, M1, M2, M4) become a prose line naming what shipped.
They are closed history, and minting retroactive issues to represent them would manufacture a record that never existed.

## Rejected alternatives

Watch the drift instead (#610).
That keeps two sources of the same fact and adds a third mechanism whose job is to keep them equal, which is the debt compounding rather than being paid.

Sort the status page by milestone number.
The numbers are creation order and rank nothing, so sorting by them discards the only ordering the file actually carries.

Renumber the milestones so number equals rank.
Every issue, journal entry and ADR that cites a milestone number would go wrong at once, and the next reprioritisation would do it again.
