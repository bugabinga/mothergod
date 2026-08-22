# ADR-0013: squash-only merges and linear history on main

Status: accepted · Date: 2026-08-22 · Proposed by the operator (issue #28)

## Context

Squash is the signing-safe merge path: GitHub creates the squash commit
server-side and signs it itself, so `required_signatures` on `main` is
satisfied regardless of the branch's own commits. The other two methods
lack that property. A rebase merge replays branch commits under new
unsigned SHAs; a merge commit drags the branch's unsigned commits into
`main` beside it.

Until now this was knowledge every agent had to hold correctly, in every
session, forever. It failed once already: PR #22 was wrongly labeled
`blocked-on-human` by a session that reasoned about signatures and got
it wrong. Documenting against a hazard (PR #25) is weaker than deleting
it.

## Decision

1. Repository settings: `allow_squash_merge` only; merge commits and
   rebase merges disabled.
2. Ruleset `gate` on `main`: `required_linear_history` added, and the
   pull-request rule's allowed merge methods reduced to `squash`, so
   the ruleset and the repo settings state the same truth.
3. Squash commit message defaults: PR title becomes the subject, PR
   body becomes the message. History keeps the reasoning, not a list of
   WIP subjects. Consequence: **write every PR body as the commit
   message the change deserves**.

## Consequences

The unsafe merge paths are unreachable, not merely documented against;
the whole category of signature reasoning disappears from agent
sessions. History becomes one commit per idea, which is what CLAUDE.md
rule 7 already implied it should look like. Conflict resolution is
unaffected: merging `main` into a PR branch stays legal, the squash
flattens it on the way in.

Caveat for the record: the rebase and merge-commit signature behavior
described in Context is reasoned from who authors the commit, observed
only for squash in this repo. Disabling them makes the claim moot,
which is the point.
