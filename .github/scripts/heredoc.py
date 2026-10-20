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
any quote and comment, names a delimiter word; the body is the lines that follow, up
to the line that is exactly the delimiter (`<<-` strips leading tabs
first); a body with no terminator runs to the end, as bash reads it. The
word is read the way bash reads it, quote removal included: `<<"E"OF` and
`<<\\E\\OF` both name `EOF`, quoted. A `<<` inside a quote or a
comment is an argument or prose, not an operator, and the lines after it
are live; the quote or comment is read across the command's own lines,
not the operator's alone. A here-string
(`<<<`) is a word, not a body. Several heredocs on one line take their
bodies in order. A word the parse cannot finish or does not model (an unclosed
quote, `<<$'EOF'`, a trailing backslash) keeps every line after it, and so
does a `$'` before the operator.

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
# an argument of that word. A backtick opens a command substitution like
# `(` does, so a shell inside one is its own receiver. An `&` is an
# operator unless it belongs to a redirection (`2>&1`, `&>file`).
SEGMENT_START = re.compile(r"(?:;|&&|\|\||\||(?<![<>])&(?!>)|\(|\{|`|^)")
ASSIGNMENT = re.compile(r"^[A-Za-z_]\w*=\S*$")
DURATION = re.compile(r"^\d+(?:\.\d+)?[smhd]?$")


def mask_quoted(text: str) -> tuple[str, bool]:
    """`text` with the contents of every quoted span and every comment
    replaced by `x`, and whether `text` ends where no operator can be read:
    inside a quote, inside a comment, or after a `$'` whose escapes this
    scan does not model. The quote characters stay. Escapes follow bash: a
    backslash quotes the next character outside quotes and before `"`, `\\`,
    `$` or a backtick inside double quotes, never inside single quotes. A
    `#` at the start of a word opens a comment that runs to the newline.
    The length is preserved."""
    out: list[str] = []
    quote = ""
    comment = False
    i = 0
    while i < len(text):
        ch = text[i]
        if comment:
            out.append("\n" if ch == "\n" else "x")
            comment = ch != "\n"
        elif quote == "'":
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
        elif ch == "'" and out and out[-1] == "$":
            return "".join(out) + "x" * (len(text) - i), True
        elif ch == "#" and (i == 0 or text[i - 1] in " \t\n;&|()<>"):
            out.append("x")
            comment = True
        else:
            out.append(ch)
            if ch in "'\"":
                quote = ch
        i += 1
    return "".join(out), quote != "" or comment


def delimiter_word(line: str, start: int) -> tuple[str, bool, int] | None:
    """The heredoc delimiter word at `line[start:]` after quote removal,
    whether any part of it was quoted, and the index after it; None when
    there is no word, a quote never closes, or the word is one this parse
    does not model: a `$` directly before a quote (`<<$'EOF'` names `EOF`
    in bash) or a backslash that ends the line (bash joins the next line
    into the word)."""
    word: list[str] = []
    quoted = False
    i = start
    while i < len(line) and line[i] not in WORD_END:
        ch = line[i]
        if ch == "\\":
            quoted = True
            i += 1
            if i == len(line):
                return None
            word.append(line[i])
        elif ch in "'\"":
            if word and word[-1] == "$":
                return None
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
    # The command's own lines, bodies excluded: a quote or a comment is read
    # across them, because bash reads it across newlines.
    code: list[str] = []
    i = 0
    while i < len(lines):
        line = lines[i]
        kept.append(line)
        code.append(line)
        i += 1
        pos = 0
        while match := OPERATOR.search(line, pos):
            pos = match.end()
            before = "\n".join(code[:-1] + [line[: match.start()]])
            masked, inside = mask_quoted(before)
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
            if not quoted or receiver(masked[len(masked) - match.start():]) not in POSTING:
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
