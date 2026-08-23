# ADR-0019: Model capability intake is a script, not an agent

Status: accepted · Date: 2026-08-23 · Serves ADR-0012

## Context

ADR-0012 makes the BDFL responsible for keeping every agent's model
ladder current. ADR-0018 gave the ladders a home in `agents/models.json`
and named the gap it did not close: a stale ladder degrades silently,
because nothing tells the BDFL that a better model exists.

The operator provisioned an Artificial Analysis API key to close it, and
raised the scoping question directly: the researcher agent's shape fits
(investigate, record a finding) but its scope does not. The researcher
exists to run compression experiments and record verdicts in
`research/`. Model-watching shares the word "research" and nothing else,
and folding it in would interleave two unrelated concerns in one seat.

## Decision

**Not an agent at all.** `.github/workflows/agent-model-intel.yml` runs
weekly with no Claude step.

Decompose the work and only one part needs a mind:

| Step | Judgement? |
|---|---|
| Call the API | No |
| Filter 600-plus models to the ladder-relevant ones | No |
| Compare scores against ladder top rungs | No, arithmetic |
| Decide whether a ladder changes | **Yes, and ADR-0012 says it is the BDFL's** |

Three reasons the first three steps are a script:

1. **Cost.** An agent would spend subscription tokens doing arithmetic.
   Run 32632168726 exhausted a seven-day window the same day this was
   designed. Adding a token consumer to perform division is the wrong
   direction.
2. **Injection surface.** The response is third-party data. A
   fixed-schema extractor pulling named numeric fields cannot be talked
   into anything; an agent reading the raw payload can.
3. **Determinism.** A benchmark number is a fact to copy, not to
   summarize. A script cannot round it, gloss it, or invent one.

### Delta-only, and the delivery channel is the point

The job posts nothing when nothing moved (ADR-0007 run economy). When a
model outside a ladder scores above that ladder's top rung, it opens or
updates one issue labeled `model-intel` and `triage`.

**An issue, specifically, because the BDFL already triages issues on
every run.** An earlier draft of this design committed a data file to the
repo and asserted the BDFL would read it on the weekly survey. Nothing
would have made that true: no prompt mentions the file, and
`agents/PERSONALITY.md` already records that a pointer to a file
demonstrably does not shape agent behavior, which is why personas are
interpolated into prompts rather than referenced. The fix is to use a
channel the agent already reads, not to add a line hoping it is noticed.

The file was dropped for a second reason found while building: every
commit on `main` arrives as a squash-merged PR, so a weekly committed
snapshot would mean a weekly automated PR for a data file. The snapshot
goes to the run's step summary and a 90-day artifact instead, which is
visible on every run without costing a review cycle.

### Deliberately out of scope for v1

**Retirement detection.** ADR-0018 left retired models unhandled on the
operator's instruction. This job cannot close it: their slugs are not
our model ids, so a ladder rung that fails to match their catalogue is
far more likely a naming mismatch than a retirement. The report says so
in those words and never treats a miss as a signal. Once the mapping is
confirmed against a real payload, disappearance detection becomes
possible; until then it would manufacture false alarms.

**Effort tiers are not new models.** Normalized matching treats
`claude-opus-5-max` as the same model as `claude-opus-5`, so an effort
tier of a rung already on the ladder does not fire. Verified against a
synthetic payload; a genuinely new id such as `claude-opus-6` does fire.

**No live payload was seen.** The key is a repository secret and is not
available outside Actions, so the extractor is written against the
documented field names, not observed ones. It is therefore
self-diagnosing: an unexpected envelope, missing fields, or unreadable
JSON produce a report naming the keys actually present, and the run exits
cleanly without posting. The first real run is the schema test.

## Consequences

No new seat, and the researcher's scope is untouched.

The judgement stays with the BDFL. This job never edits
`agents/models.json`; it puts evidence on the table and files it where
the BDFL already looks.

**The gap it does not close.** If the comparison never fires, the BDFL
never hears, and a stale ladder stays stale. ADR-0018's gap is moved
rather than eliminated. The improvement is only that an issue which never
appears is something you can go looking for, where a file nobody opens
looks identical whether it is current or a year old.

Attribution to Artificial Analysis is required on every tier of their
API, so it is written into the generated report rather than left to
whoever quotes it. Redistribution is a separate permission: their data
must not reach mothergod.dev without asking them first.
