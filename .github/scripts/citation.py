"""Bare commit hashes in a body about to be posted: the A2 predicate, one source.

CLAUDE.md Voice A2: a commit cited as a thing is `sha ("subject")`, never a
bare hash; a head named as the pin a verdict or merge covers is not a
citation. The rule rides every seat's prompt and still slipped four times
in two days (2026-11-07 to 2026-11-08): PR #1937's body, the curator's
replies on #1921 and #1926, the herald's body on #1955; three seats, each
read by hand in the retrospect. A rule a writer has to remember on every
post is a rule that will eventually not be applied (gh-comment's
docstring, #138). This module is the predicate; `gh-comment` and `gh-pr`
run it at the boundary the body leaves through, while the writer is still
present to fix the sentence (the line deny-off-voice draws).

Bare means: 7 to 40 hex characters carrying at least one letter and one
digit, on word boundaries, outside inline code, fenced blocks and HTML
comments, not inside a URL or an action pin (`/` or `@` before it), not
the first half of an `a..b` range, and not followed by ` ("`, the cited
form. The pin exception is spelled by backticks: a pin is a literal
identifier and rides as code, a citation is prose and carries its subject.
A 64-hex verdict stamp is longer than 40 and never matches; a run id is
decimal and never matches; a hex-only word ("effaced") carries no digit
and never matches.

Where the writer is present (`CLAUDECODE` in the environment, set in every
model session's shell) the caller refuses the post and names each hash.
A workflow step posting a fixed body (agent-review.yml's refusal notice)
has no writer to fix it, so the caller warns and posts. Liveness: the
retrospect prints every posted artifact, so a bare hash there after
2026-11-08 means this stopped firing.
"""

import os
import re

FENCE = re.compile(r"```.*?```", re.DOTALL)
INLINE = re.compile(r"`[^`\n]*`")
COMMENT = re.compile(r"<!--.*?-->", re.DOTALL)
HASH = re.compile(
    r"(?<![\w/@.])(?=[0-9a-f]*[a-f])(?=[0-9a-f]*\d)[0-9a-f]{7,40}(?!\w)(?!\.\.)(?! \(\")"
)


def bare_hashes(body: str) -> list[str]:
    """Each bare hash in `body`, in first-occurrence order, no repeats."""
    prose = COMMENT.sub(" ", INLINE.sub(" ", FENCE.sub(" ", body)))
    found: list[str] = []
    for match in HASH.finditer(prose):
        if match.group() not in found:
            found.append(match.group())
    return found


def complaint(hashes: list[str]) -> str:
    """The one line a caller prints (after its own prog prefix), naming each hash and both fixes."""
    plural = "es" if len(hashes) > 1 else ""
    return (
        f"bare commit hash{plural} {', '.join(hashes)}: a commit cited as a thing "
        'is sha ("subject"), a pin rides in backticks (CLAUDE.md Voice A2)'
    )


def writer_present() -> bool:
    """True inside a model session's shell, where a refusal reaches someone who can fix it."""
    return bool(os.environ.get("CLAUDECODE"))
