# ADR-0076: The finals reports have one writer, the publish PR cut from the pass on main

Status: accepted · Date: 2026-10-20 · Serves issue #1437 · Retires the guard of issue #327

## Context

`baseline_gate check` demanded that both finals reports embed the fingerprint of `bench/baseline.json` (issue #327).
A codec PR whose output moved a pinned bpb therefore regenerated the finals inside the PR: the seat dispatched `bench-report.yml` on its branch, waited a wake, downloaded, and pushed five files.
The merge then touched `bench/baseline.json` and ran the pass again on main, whose output the detector never published, because the squash commit touches the finals and names no generator (issue #1381).
Two passes per byte-moving change, and the in-branch finals carry the race of #1381: generated at the branch's base, dated at the merge.
The in-PR demand also saw only the 11 cases of 50,000 bytes: #1423 moved six Silesia finals and no pinned bpb, and the guard demanded nothing (issue #1434).

## Decision

`check` keeps pinning bpb and digest for the 11 cases and stops demanding the finals.
`stale_finals_reports`, `finals::stale_reason`, the embedded `baseline-fingerprint` marker, `baseline::fingerprint` and `load_and_fingerprint`, and the stderr recipe issue #1346 found wrong are deleted.
The finals' only writer is the publish PR the heartbeat ships from the pass on main (`bench-report --publish`), whose title names the generator.
A codec PR commits `bench/baseline.json` and nothing under `docs/benchmarks/`.

## Consequences

The window between a byte-moving merge and its publish is hours and honest: `bench-report` reads `running`, then `publish`, never `fresh`, and `state` prints that line every wake.
"Fresh" is derivable from the repo alone: one generator, on main, after the merge, named in its commit.
A codec PR no longer waits a wake for a runner pass, and the finals pass runs once per byte-moving change.
Issue #1381 closes by deleting the detector's self-generator branch; `bench-report.yml`'s header and `agent-heartbeat.yml`'s dispatch-on-branch comment lose that path in the same follow-up, those two files being the BDFL's.
The two committed reports keep their marker line until the next publish regenerates them, and nothing reads it.
A reports commit naming no generator is stale by rule, never fresh (issue #1381), so a hand edit of a report heals through the next publish; an UNREADABLE answer was rejected because `--publish` needs state publish and a revert names no generator either, a deadlock on the one commit nothing honest writes.

## Rejected alternatives

**Keep the in-PR demand and teach the detector the branch's base.**
The base is lost at squash, and a title convention is a prompt rule three seats would each learn once.

**An in-PR finals run as a required check.**
212 MB of Silesia at 0.188 MB/s (`bench-report.yml`'s timed pass of 2026-10-18) to 0.216 MB/s (`docs/benchmarks/silesia.md`'s aggregate row) is 16 to 19 minutes on every push of every codec PR, for a number the post-merge pass measures once.

This is a guard moved, not weakened: the in-PR demand saw a bpb move on 50,000-byte cases and nothing else, and the post-merge pass measures the finals themselves.
