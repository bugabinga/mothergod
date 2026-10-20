"""Drop the heredoc bodies a posting script reads, before a guard scans a command.

`deny-ci-background` and `deny-review-recompute` match a command string
with no notion of quoting or nesting, a bluntness #355's review settled:
every anchoring cheap enough for a hook left a live idiom unmatched. The
cost was the prose. A verdict saying `cargo x lint` was not run, a commit
message with "fmt & clippy" in it, each fed to a posting script through a
heredoc, tripped the guard it never endangered, and the seat rewrote the
sentence and posted again: 45 denied turns a week across the reviewer and
three other seats (#1377).

A heredoc body is data for the command that reads it. When that command
is one of this repo's posting scripts (`POSTING`), the data is posted or
committed, never executed, and nothing in it can run or detach. So the
body leaves the string before the scan, and the rest of the command, the
operator line and whatever follows the terminator, stays in it.

Every other receiver keeps its body. `bash <<EOF` runs the body, and so
does `timeout 600 bash <<EOF` or `env sh <<EOF`, which is why the list is
of receivers that never execute and not of shells that do: an unknown
receiver fails closed, the posture #355 chose, and a wrapper around a
shell still ends in a shell. Two prefixes are transparent, because they
change nothing about who reads the body: `NAME=value` assignments and
`timeout [flags] <duration>`, the wrapper CLAUDE.md rule 12 asks for.

The parse is line-based, like bash's: the operator `<<`, `<<-`, or either
with a quoted or `\\`-escaped word, names a delimiter; the body is the
lines that follow, up to the line that is exactly the delimiter (`<<-`
strips leading tabs first); a body with no terminator runs to the end,
as bash reads it. A here-string (`<<<`) is a word, not a body. Several
heredocs on one line take their bodies in order. A `<<` inside quotes on
the operator line reads as an operator too, which over-strips for a
posting receiver and never for any other: less scanned, nothing run.
"""

import os
import re

# The scripts whose stdin is a body they post or commit and never execute.
POSTING = frozenset({"gh-comment", "gh-pr", "tg-send", "push-branch", "post-verdict"})

# `<<` or `<<-`, not part of `<<<`, then the delimiter word: single-quoted,
# double-quoted, or bare (a leading backslash quotes it too).
OPERATOR = re.compile(
    r"(?<!<)<<(?!<)(-?)\s*(?:'([^']*)'|\"([^\"]*)\"|(\\?[^\s;&|<>()]+))"
)

# The word that reads the body: the first word of the operator's own
# simple command, after the last control operator or opening bracket on
# the line, skipping assignments and a timeout wrapper. Everything else is
# an argument of that word.
SEGMENT_START = re.compile(r"(?:;|&&|\|\||\||&|\(|\{|^)")
ASSIGNMENT = re.compile(r"^[A-Za-z_]\w*=\S*$")
DURATION = re.compile(r"^\d+(?:\.\d+)?[smhd]?$")


def without_posted_bodies(command: str) -> str:
    """`command` with every heredoc body a posting script reads removed."""
    lines = command.split("\n")
    kept: list[str] = []
    i = 0
    while i < len(lines):
        line = lines[i]
        kept.append(line)
        i += 1
        for match in OPERATOR.finditer(line):
            delimiter = match.group(2) or match.group(3) or match.group(4).lstrip("\\")
            strip_tabs = match.group(1) == "-"
            end = i
            while end < len(lines):
                candidate = lines[end].lstrip("\t") if strip_tabs else lines[end]
                if candidate == delimiter:
                    break
                end += 1
            if receiver(line[: match.start()]) not in POSTING:
                kept.extend(lines[i:end])
            if end < len(lines):
                kept.append(lines[end])
            i = end + 1
    return "\n".join(kept)


def receiver(prefix: str) -> str:
    """The basename of the command word that owns a heredoc operator at the
    end of `prefix`, or "" when the line holds no word before it."""
    segment = prefix[max(m.end() for m in SEGMENT_START.finditer(prefix)):]
    words = segment.split()
    while words and ASSIGNMENT.match(words[0]):
        words.pop(0)
    if words and os.path.basename(words[0]) == "timeout":
        words.pop(0)
        while words and words[0].startswith("-"):
            # `-k` and `-s` take the next word; the long forms join with `=`.
            if words.pop(0) in ("-k", "-s") and words:
                words.pop(0)
        if words and DURATION.match(words[0]):
            words.pop(0)
    return os.path.basename(words[0]) if words else ""
