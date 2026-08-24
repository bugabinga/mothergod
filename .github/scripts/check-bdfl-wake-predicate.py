#!/usr/bin/env python3
"""Fail if agent-bdfl.yml's two wake-predicate copies have drifted (issue #145).

concurrency.group and jobs.bdfl.if MUST mirror each other (issue #46); GitHub
gives no way to single-source them (`on:` cannot filter by actor,
`concurrency` cannot read job state), so this is the substitute check.
"""

import re
import sys

import yaml

WORKFLOW = ".github/workflows/agent-bdfl.yml"
MARKER = "&& 'agent-bdfl'"


def normalize(expr):
    return re.sub(r"\s+", " ", expr.replace("${{", "").replace("}}", "")).strip()


def main():
    with open(WORKFLOW) as f:
        doc = yaml.safe_load(f)

    group_expr = doc["concurrency"]["group"]
    if_expr = doc["jobs"]["bdfl"]["if"]

    group_norm = normalize(group_expr)
    if MARKER not in group_norm:
        print(f"{WORKFLOW}: expected marker {MARKER!r} not found in concurrency.group; "
              "the split this check relies on is gone, fix the check first.")
        return 1
    predicate = group_norm.split(MARKER)[0].strip()

    if predicate != normalize(if_expr):
        print(f"{WORKFLOW}: wake predicate copies have drifted.\n"
              f"  concurrency.group predicate: {predicate}\n"
              f"  jobs.bdfl.if:                {normalize(if_expr)}")
        return 1

    print("wake predicate copies match")
    return 0


if __name__ == "__main__":
    sys.exit(main())
