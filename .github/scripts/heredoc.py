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
is one of this repo's posting scripts (`POSTING`) and the delimiter is
quoted, the data is posted or committed, never executed, and nothing in
it can run or detach. So the body leaves the string before the scan, and
the rest of the command, the operator line and whatever follows the
terminator, stays in it.

An unquoted delimiter keeps its body: bash expands `$(...)` and backticks
in that body before the posting script reads a byte, so the receiver does
not make it inert. Prose naming a gate in backticks is the common case,
and it runs. The deny messages tell a seat to write `<<'EOF'`.

Every other receiver keeps its body. `bash <<EOF` runs the body, and so
does `timeout 600 bash <<EOF` or `env sh <<EOF`, which is why the list is
of receivers that never execute and not of shells that do: an unknown
receiver fails closed, the posture #355 chose, and a wrapper around a
shell still ends in a shell. Two prefixes are transparent, because they
change nothing about who reads the body: `NAME=value` assignments and
`timeout [flags] <duration>`, the wrapper CLAUDE.md rule 12 asks for.

The parse is line-based, like bash's: the operator `<<` or `<<-`, outside
any quote, names a delimiter word; the body is the lines that follow, up
to the line that is exactly the delimiter (`<<-` strips leading tabs
first); a body with no terminator runs to the end, as bash reads it. The
word is read the way bash reads it, quote removal included: `<<"E"OF` and
`<<\\E\\OF` both name `EOF`, quoted. A `<<` inside a quote is an
argument, not an operator, and the lines after it are live. A here-string
(`<<<`) is a word, not a body. Several heredocs on one line take their
bodies in order. A word the parse cannot finish (an unclosed quote) keeps
every line after it.

The receiver is read from the operator's prefix with quoted spans masked,
so a `;` or a command name inside an argument names no segment.
"""

import os
import re

# The scripts whose stdin is a body they post or commit and never execute.
POSTING = frozenset({"gh-comment", "gh-pr", "tg-send", "push-branch", "post-verdict"})

# `<<` or `<<-`, not part of `<<<`; the delimiter word follows and is read
# by `delimiter_word`.
OPERATOR = re.compile(r"(?<!<)<<(?!<)(-?)[ \t]*")

# Characters that end an unquoted word.
WORD_END = frozenset(" \t;&|<>()")

# The word that reads the body: the first word of the operator's own
# simple command, after the last control operator or opening bracket on
# the line, skipping assignments and a timeout wrapper. Everything else is
# an argument of that word.
SEGMENT_START = re.compile(r"(?:;|&&|\|\||\||&|\(|\{|^)")
ASSIGNMENT = re.compile(r"^[A-Za-z_]\w*=\S*$")
DURATION = re.compile(r"^\d+(?:\.\d+)?[smhd]?$")


def mask_quoted(text: str) -> tuple[str, bool]:
    """`text` with the contents of every quoted span replaced by `x`, and
    whether `text` ends inside a quote. The quote characters stay. Escapes
    follow bash: a backslash quotes the next character outside quotes and
    before `"`, `\\`, `$` or a backtick inside double quotes, never inside
    single quotes."""
    out: list[str] = []
    quote = ""
    i = 0
    while i < len(text):
        ch = text[i]
        if quote == "'":
            out.append(ch if ch == "'" else "x")
            if ch == "'":
                quote = ""
        elif ch == "\\" and i + 1 < len(text) and (not quote or text[i + 1] in '"\\$`'):
            out.append(ch if not quote else "x")
            out.append("x")
            i += 1
        elif quote == '"':
            out.append(ch if ch == '"' else "x")
            if ch == '"':
                quote = ""
        else:
            out.append(ch)
            if ch in "'\"":
                quote = ch
        i += 1
    return "".join(out), quote != ""


def delimiter_word(line: str, start: int) -> tuple[str, bool, int] | None:
    """The heredoc delimiter word at `line[start:]` after quote removal,
    whether any part of it was quoted, and the index after it; None when
    there is no word or a quote never closes."""
    word: list[str] = []
    quoted = False
    i = start
    while i < len(line) and line[i] not in WORD_END:
        ch = line[i]
        if ch == "\\":
            quoted = True
            i += 1
            if i < len(line):
                word.append(line[i])
        elif ch in "'\"":
            quoted = True
            i += 1
            while i < len(line) and line[i] != ch:
                if ch == '"' and line[i] == "\\" and i + 1 < len(line) and line[i + 1] in '"\\$`':
                    i += 1
                word.append(line[i])
                i += 1
            if i == len(line):
                return None
        else:
            word.append(ch)
        i += 1
    if i == start:
        return None
    return "".join(word), quoted, i


def without_posted_bodies(command: str) -> str:
    """`command` with every heredoc body a posting script reads removed."""
    lines = command.split("\n")
    kept: list[str] = []
    i = 0
    while i < len(lines):
        line = lines[i]
        kept.append(line)
        i += 1
        pos = 0
        while match := OPERATOR.search(line, pos):
            pos = match.end()
            prefix, inside = mask_quoted(line[: match.start()])
            if inside:
                continue
            parsed = delimiter_word(line, match.end())
            if parsed is None:
                kept.extend(lines[i:])
                return "\n".join(kept)
            delimiter, quoted, pos = parsed
            strip_tabs = match.group(1) == "-"
            end = i
            while end < len(lines):
                candidate = lines[end].lstrip("\t") if strip_tabs else lines[end]
                if candidate == delimiter:
                    break
                end += 1
            if not quoted or receiver(prefix) not in POSTING:
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
