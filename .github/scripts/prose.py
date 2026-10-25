"""The files CLAUDE.md's Voice governs, and the one character it bans, in one place.

`deny-em-dash` (the Edit and Write boundary, in a session) and `em-dash-diff`
(ci.yml's `em-dash` job, on the diff) apply the no-em-dash rule to the same
files, so the set is defined once: markdown, the research log's JSON lines,
and TOML, the three places prose lives that are not code. Code files carry
the character as data (deny-em-dash's own tests, tg-send's swap,
prompt-bytes' count) and stay out; a prompt in a workflow YAML is prose, and
prompt-bytes already counts its em dashes.

The escape form counts beside the literal, because TOML and JSON lines
decode `\\u2014` into the same character on the page.
"""

import re

SUFFIXES = (".md", ".jsonl", ".toml")

EM_DASH = re.compile(r"—|\\u2014")


def is_prose(path: str) -> bool:
    """True when `path` ends in one of SUFFIXES."""
    return path.endswith(SUFFIXES)


def carries(text: str) -> bool:
    """True when `text` carries an em dash, literal or as the `\\u2014` escape."""
    return EM_DASH.search(text) is not None
