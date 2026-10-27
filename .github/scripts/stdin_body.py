"""Read a posting script's body from stdin, or die when nothing is piped.

`gh-comment`, `gh-pr`, `post-verdict`, `push-branch` and `tg-send`
(`heredoc.POSTING`) each take their body on stdin. Inside a Claude Code
Bash tool, stdin is an open pipe that never closes, so a call that
forgot the body blocked on the read until the harness timeout: ten
minutes of a 948 s maintainer session in run 40348770291, with no trace
a judge sees (#1682). Any real body is readable at once, a heredoc and
a printf included, so a pipe still silent after WAIT_S is a call with
no body, and `read` dies naming the fix instead of waiting.

`select` reports EOF as readable, so a closed empty stdin returns ""
and the caller's own empty-body message fires, unchanged.
"""

import os
import select
import sys

# Long enough for any writer that exists to produce its first byte: a
# body is composed before the call, never generated during it.
WAIT_S = 10


def read(what="body", wait_s=WAIT_S):
    """Return stdin stripped, or exit 1 when nothing arrives in `wait_s` seconds."""
    ready, _, _ = select.select([sys.stdin], [], [], wait_s)
    if not ready:
        prog = os.path.basename(sys.argv[0])
        print(f"{prog}: the {what} comes from stdin; nothing arrived in "
              f"{wait_s:g} s: pipe it (a heredoc, <<'EOF', or a printf is "
              "readable at once)", file=sys.stderr)
        sys.exit(1)
    return sys.stdin.read().strip()
