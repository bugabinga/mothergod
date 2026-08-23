# ADR-0020: Python is allowed in CI glue, and stops hiding in YAML

Status: accepted · Date: 2026-08-23 · Amends ADR-0006 (scope of the language rule)

## Context

ADR-0006 says "No new Python (or other-language) code enters the
repository." Its reasoning is entirely about the codec: a second
implementation reopens the two-codec gap that produced the project's
canonical port bug, and the Python proxy mis-measured twice on its own.
Every clause is about the product and the tooling that measures it.

The agent machinery ignored the letter of that rule from the start, but
sideways. `agent-guard`, `agent-pause`, and `agent-audit` all embed
substantial Python in shell heredocs inside their `action.yml`. It is
Python in the repository; it is simply not in a `.py` file.

Building `agent-model-intel` (ADR-0019) I followed that precedent
deliberately, wrote roughly 150 lines of extractor, and inlined it to
avoid the rule. Operator's verdict on reading it: "circumventing the
python rule by hiding it in workflow is ass." Correct. A rule dodged by
choosing a worse file format is not being honoured, and the dodge costs
real things: no syntax highlighting, no linting, no direct test
execution, and a diff that reviewers read through YAML indentation.

## Decision

ADR-0006 clause 1 narrows to what its reasoning actually supports.

**Rust only, unchanged, for the product and its scaffold:** the codec,
experiments, benchmark harness, corpus generators, and anything whose
output is a number this project publishes or a bitstream it ships. The
two-codec gap and the proxy's measurement errors are the reasons, and
they apply with full force here. Clauses 2, 3 and 4 of ADR-0006 stand
untouched: the Python loop is not resumed, the archive stays in history,
and its proxy numbers remain historical context only.

**CI glue and one-off scripts may be Python.** Workflow support code,
audit extraction, limit routing, ladder resolution, data intake. None of
it touches the codec, none of it produces a published number, and none of
it can reopen the gap ADR-0006 exists to prevent.

**Scripts live in files, not in YAML heredocs.** New CI Python goes in
`.github/scripts/`. Existing inline Python in the composite actions is not
swept as part of this decision; it moves when those actions are next
touched for another reason, because a mass rewrite of working
limit-handling machinery is a worse risk than the untidiness it fixes.

## Consequences

`.github/scripts/model-intel.py` becomes a real file: runnable directly
against a fixture, reviewable as a code diff instead of through YAML
indentation, and open to a linter if one is ever added. No Python linter
runs in CI today and this decision does not add one, so "lintable" is a
property of the file, not a check it passes. Its behaviour is unchanged
by the move: all eight extractor cases produce identical verdicts before
and after.

The boundary is a property of the artifact, not a line count: does this
code produce or measure the compressor? Rust. Does it move data around
the machinery that runs the project? Either, and pick the one that makes
the code clearest.

Risk accepted: the boundary is a judgement rather than a mechanical test,
so it can be argued about at the edges. The failure mode to watch for is
Python creeping toward the corpus or the benchmark harness, where
ADR-0006's original reasoning still binds completely. If a script starts
producing numbers that end up in `research/`, it is on the wrong side of
the line and should have been Rust.
