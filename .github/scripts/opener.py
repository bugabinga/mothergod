#!/usr/bin/env python3
"""Take the completion announcement off the front of a final response.

Imported by run-notice.py, which drops it from the operator's notice, and
by `retrospect`, which flags it beside the session that wrote it. Not run.

CLAUDE.md's Voice rules have forbidden opening a final response with
"Heartbeat done." or "Run complete. Summary:" since the operator reported
every notice reading alike (2026-09-13). The rule is prose, and prose is
what the model gets wrong: sixteen of forty-nine audited responses from
2026-09-19 to 2026-09-21 opened that way, four of the eight in a single
retrospect window. The first line of a notice is the entire message to a
reader who stops there (run-notice.py), so the opener is what the operator
read instead of the result. Hot, wrong a third of the time, on a substrate
one regex reads: the ADR-0022 test for compiling the rule.

The predicate is the sentence, not the sentiment. The first sentence, cut
at the first `.`, `!`, `?` or labelling `:`, is an announcement when it has
at most twelve words, carries one of done/complete/completed/finished/sent/
posted, and carries none of the marks that make a sentence content: a
digit, `#`, a backtick, a bracket, a link. "PR #641 passes." and "Merged
#545." pass through untouched; "Heartbeat's one substantial unit of work
is done and posted." does not. A bare label the announcement introduced
("Summary:") goes with it. Trimming repeats while the front is still an
announcement, and never empties the response: "Heartbeat complete for this
cycle." alone stays, because a notice that says nothing is more honest
than one that says less than nothing. Calibrated against those forty-nine
responses: sixteen trimmed, no content sentence lost.

What this does NOT catch, said plainly: an announcement without the
keyword ("Inbox still empty. That closes out this wake."), which reads as
content here and stays; a second announcement that would leave nothing
behind; and anything past the opener, because the opener is the defect and
the rest of the response is the auditor's to judge.
"""

import re

MAX_WORDS = 12
KEYWORD = re.compile(r"\b(done|complete|completed|finished|sent|posted)\b", re.I)
# A digit, an issue or PR reference, code, a bracketed link or a URL: any
# one of them means the sentence says something.
CONTENT = re.compile(r"[0-9#`\[]|https?://")
TERMINATOR = re.compile(r"[.!?:](?=\s|$)")
# "Summary:" or "**Summary:**", alone on its line.
LABEL = re.compile(r"^\**[A-Za-z][A-Za-z ]{0,24}:\**$")


def announcement(sentence):
    """True for a short, contentless sentence that says the run is over."""
    return (
        len(sentence.split()) <= MAX_WORDS
        and KEYWORD.search(sentence) is not None
        and CONTENT.search(sentence) is None
    )


def split_first(text):
    """The first sentence and what follows it; no terminator means no split."""
    match = TERMINATOR.search(text)
    if match is None:
        return text, ""
    return text[: match.end()], text[match.end():].lstrip()


def trim(text):
    """The response without its announcement opener, and the sentences removed.

    The second value is empty when nothing was trimmed, so a caller can
    treat it as the finding it is.
    """
    removed = []
    text = text.strip()
    while True:
        first, rest = split_first(text)
        if not announcement(first):
            break
        line, _, tail = rest.partition("\n")
        if LABEL.match(line.strip()):
            rest = tail.lstrip()
        if not rest:
            break
        removed.append(first)
        text = rest[0].upper() + rest[1:]
    return text, removed
