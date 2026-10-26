"""One more try for a gh write GitHub answered with a 5xx, after a pause and a look at what landed.

Three reviewer runs on 2026-10-26 (40374215944, 40375225550, 40376256921)
each lost a verdict post to a GitHub 502 or 504 inside one half hour, and
each did the same thing by hand: read the thread, see that nothing had
landed, call post-verdict again. A step a seat performs three times in
one window is a mechanism, not a judgment (ADR-0022), and this is it, in
the writes a verdict consists of: gh-comment's post and labels, and
post-verdict's label flip.

`transient` reads gh's stderr for the shapes a 5xx takes: `HTTP 502`,
`Server Error`, `Bad Gateway`, `Gateway Timeout`, `Service Unavailable`,
and GraphQL's `Something went wrong while executing your query`, the
API's timeout inside a 200 envelope. A 4xx is never retried: a 401 is
the hour (#81), a 404 or a 422 is the caller's, and a second try changes
nothing.

`once` runs the write. On a transient failure it first asks `landed`,
when the caller gave one, whether the first try took effect despite the
answer, and a URL from it stands as the write's own result, so a comment
GitHub stored and then failed to acknowledge is never posted twice.
Otherwise it pauses and runs the write one more time. One retry, not a
loop: a degradation that outlasts the pause is the next wake's rescue
(stalled-prs), and a loop here would hold a session that rule 12 says
must not wait. The pause is GH_RETRY_DELAY seconds, 20 by default; the
tests set it to 0.
"""

import os
import re
import subprocess
import sys
import time

TRANSIENT = re.compile(
    r"HTTP 5\d\d|Server Error|Bad Gateway|Gateway Time-?out|Service Unavailable"
    r"|Something went wrong while executing your query",
    re.IGNORECASE,
)


def transient(stderr):
    """Whether gh's stderr names a 5xx or the GraphQL timeout."""
    return bool(TRANSIENT.search(stderr or ""))


def delay():
    """Seconds between the two tries: GH_RETRY_DELAY, or 20."""
    try:
        return float(os.environ.get("GH_RETRY_DELAY", "20"))
    except ValueError:
        return 20.0


def once(write, landed=None, name="the write", note=lambda line: print(line, file=sys.stderr)):
    """Run `write` (a callable returning a CompletedProcess); on a 5xx, one more try.

    `landed`, when given, is called first and returns the URL of the
    first try's effect or None; a URL comes back as a succeeded process
    with that URL on stdout. `note` takes each line this prints.
    """
    result = write()
    if result.returncode == 0 or not transient(result.stderr):
        return result
    lines = (result.stderr or "").strip().splitlines()
    reason = lines[-1] if lines else f"exit {result.returncode}"
    if landed is not None:
        url = landed()
        if url:
            note(f"{name} answered a 5xx ({reason}) but had landed: {url}")
            return subprocess.CompletedProcess(result.args, 0, url + "\n", "")
    pause = delay()
    note(f"{name} answered a 5xx ({reason}); one more try in {pause:g} s")
    time.sleep(pause)
    return write()
