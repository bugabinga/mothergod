#!/usr/bin/env python3
"""Say whether an unanswered operator message is sitting in the Telegram inbox.

Reads a Cloudflare KV "list a namespace's keys" response on stdin and prints one
word: `waiting`, `empty`, or `unreadable`.

`unreadable` is a distinct answer on purpose. A gate that cannot read the inbox
and answers "empty" tells the operator that nobody is waiting while they are
waiting, and it looks identical to the truth in the run log. That is the failure
this file exists to make impossible.

Always exits 0. The caller is a workflow step under `bash -e` whose job is to
start a progress indicator; crashing there would fail a whole BDFL run over a
chat nicety. The third verdict is how a problem gets reported instead.

Inbox keys are zero-padded update ids prefixed `u:` (issue #5). `chatlog` and
anything else living in the namespace is not an unanswered message.

Usage: python3 .github/scripts/inbox-state.py < listing.json
"""

import json
import sys

PREFIX = "u:"


def verdict(raw: str) -> str:
    """Classify a listing response. Never raises: every input has an answer."""
    try:
        body = json.loads(raw)
        if not body.get("success"):
            return "unreadable"
        keys = body["result"]
        return "waiting" if any(k.get("name", "").startswith(PREFIX) for k in keys) else "empty"
    except (AttributeError, KeyError, TypeError, ValueError):
        # Every shape that is not the documented one lands here: malformed JSON
        # (ValueError), a non-object top level or entries that are not objects
        # (AttributeError), a missing `result` (KeyError), a `result` that is
        # not iterable (TypeError). Enumerated rather than a bare `except`, so a
        # genuine bug in this file still surfaces as a crash instead of quietly
        # becoming "unreadable" forever.
        return "unreadable"


if __name__ == "__main__":
    print(verdict(sys.stdin.read()))
