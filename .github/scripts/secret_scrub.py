"""Redact credential-shaped strings from text on its way off this machine.

One scrubber, imported by every exit: the audit artifact (audit-extract.py),
issue and PR bodies (gh-comment) and Telegram (tg-send). It used to live only
in audit-extract, under a comment claiming artifacts were "the one point where
session text leaves the machine". That was false. Issue comments, PR bodies and
commit contents are unmasked on a public repo too, as CLAUDE.md hard rule 10
says outright, and nothing scrubbed them.

Two rules, and the order they are in matters.

`URL_USERINFO` is the load-bearing one, because it is shaped, not prefixed.
Issue #425 added prefix patterns to catch an app installation token echoed out
of the git remote URL by `git remote -v`. On 2026-09-18 the same leak recurred
(the herald's run) and the patterns matched NOTHING: the runner had begun
minting a 390-character token carrying `_` and `-`, and every prefix pattern
was pinned to the 40-character `ghs_` form. The defence had rotted silently for
weeks because its only test fed it a synthetic `ghs_` fake, so the test proved
the regex matched the fixture and never that it matched the token the runner
actually mints. Userinfo in an http(s) URL is a credential by construction, at
any length, in any format, so this rule cannot go stale the same way.

The prefix rules stay as defence in depth: they catch a bare token pasted
outside a URL, which the userinfo rule by definition cannot see.

The remote URL itself, the source of both leaks, is cleaned at session
start by the scrub-remote hook (#597); this scrubber stays as the exit-side
defence for whatever else echoes a credential.

Unprefixed secrets (Cloudflare's bare-alnum token) are indistinguishable from
ordinary prose and must be passed in as `values`; no pattern can find them
without redacting the whole document.
"""

import re

REDACTION = "***REDACTED***"

# Credentials in URL userinfo: `https://<anything>@host`. `[^/\s@]+` keeps the
# match inside the authority, so a path containing `@` and a bare email beside
# an ordinary link are both left alone.
URL_USERINFO = re.compile(r"(?i)\b(https?://)[^/\s@]+@")

TOKEN_PATTERNS = [
    # GitHub: ghp_ personal, gho_ OAuth, ghu_/ghs_ app, ghr_ refresh.
    re.compile(r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{20,}"),
    re.compile(r"\bgithub_pat_[A-Za-z0-9_]{20,}"),
    # Anthropic API keys.
    re.compile(r"\bsk-ant-[A-Za-z0-9_-]{20,}"),
    # Telegram bot tokens, bare or in the /bot<token>/ URL form the Bot API
    # forces (hard rule 10). Lookbehind, not \b: `t` to digit in `bot123...`
    # is no word boundary, and that URL form is the documented leak shape.
    re.compile(r"(?<!\d)\d{6,}:[A-Za-z0-9_-]{30,}"),
]


def scrub(text, values=()):
    """Redact `values` and every credential shape from `text`.

    Returns `(scrubbed, count)`. `count` is how many redactions were made, so a
    caller can tell the agent it just wrote a secret instead of swallowing it:
    a silent redaction hides a hard-rule-10 violation the session should report.

    Only values of 8 characters or more are honoured. A shorter "secret" is a
    misconfiguration, and redacting a 1-character value would erase the text.
    """
    count = 0
    for value in values:
        value = value.strip()
        if len(value) >= 8 and value in text:
            text = text.replace(value, REDACTION)
            count += 1
    text, found = URL_USERINFO.subn(r"\1" + REDACTION + "@", text)
    count += found
    for pattern in TOKEN_PATTERNS:
        text, found = pattern.subn(REDACTION, text)
        count += found
    return text, count
