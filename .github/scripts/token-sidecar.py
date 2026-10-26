#!/usr/bin/env python3
"""Re-mint the claude app token every 45 minutes beside a session, the newest in a file the writers read (#1687).

The action mints one app token when the job starts and GitHub expires it
an hour later, inside a job that may run four (GOVERNANCE "Token
lifetime"). Three maintainer runs in 14 days outlived it and ended as
handoffs, a verified diff posted for a second session to ship, because
the PR open is the one write with no late path. Four fixes routed writes
around the hour; this removes the hour. Measured before it was built: the
exchange re-mints inside one job (run 40376718110, two mints 120 s apart)
and the runner's OIDC request token outlives the hour (run 40377179460,
a mint 65 minutes after the first).

Every model seat but the reviewer starts this in its own session before
the Claude step, on the seam vitals-sidecar.py cut (#1666): setsid,
stdout and stderr to a file under RUNNER_TEMP, the runner's OIDC request
token on stdin and scrubbed from the environment, because a process that
lives beside the session for hours would otherwise hold it in
/proc/<pid>/environ. The request URL stays in the environment: without
the bearer it opens nothing. The first mint is at --interval, never at
the start: until then the action's own token is as fresh, so a run that
ends inside the interval, most of them, mints nothing and leaves no token
behind it. From then on, every --interval minutes: a fresh OIDC token
from the runner, one POST to the exchange, the app token written whole
to --path, a temporary file at mode 0600 renamed over the last, so a
reader sees the old token or the new and never a partial one. The
writers, gh-pr, push-branch, merge-pr and settle-push, call
app_token.prefer_file at each gh call, so a write rides a token at most
--interval minutes old whatever the session's age. A mint that fails
keeps the last file, a stale token being today's failure and not a new
one, prints the statuses, and tries again after --retry seconds. Nothing
is revoked: a revoke on re-mint races a write that read the file a
moment earlier, the hour bounds every token, and the runner is destroyed
with the file.

Never prints a token, never fails the run, never holds it: exit 0 on
every path, a failed mint is one line of status codes, and the loop ends
at --max-minutes, the longest any seat's timeout-minutes allows, should
the job's orphan cleanup miss it. An empty stdin is one line and exit 0.

Usage: printf '%s\n' "$ACTIONS_ID_TOKEN_REQUEST_TOKEN" | token-sidecar.py --path FILE [--interval MINUTES] [--retry SECONDS] [--max-minutes N] [--rounds N] [--exchange-url URL]

--rounds ends the loop after N mints and --exchange-url points it at a
fake; the test suite sets both, no workflow does.
"""

import argparse
import os
import sys
import time

import app_token


def write(path, token):
    """The token to `path` whole: written to a 0600 temporary beside it, then renamed over it."""
    tmp = f"{path}.tmp"
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        f.write(token + "\n")
    os.replace(tmp, path)


def mint(request_url, request_token, exchange_url):
    """(token or None, detail): one mint, the detail a token-free line of statuses."""
    status, oidc = app_token.oidc_token(request_url, request_token)
    if oidc is None:
        return None, f"oidc {status}, no token in the response"
    status, token, body = app_token.exchange(oidc, url=exchange_url)
    if token is None:
        return None, f"oidc 200, exchange {status}, {app_token.reason(body)}"
    return token, f"exchange {status}, expires_at {body.get('expires_at') or 'not stated'}"


def parse(argv):
    parser = argparse.ArgumentParser(prog="token-sidecar.py", description=__doc__.splitlines()[0])
    parser.add_argument("--path", required=True, help="where the newest app token goes, whole, mode 0600")
    parser.add_argument("--interval", type=float, default=45.0, help="minutes between mints, and before the first (45)")
    parser.add_argument("--retry", type=float, default=60.0, help="seconds before the next try after a failed mint (60)")
    parser.add_argument("--max-minutes", type=float, default=300.0,
                        help="stop after this long, the longest seat's timeout-minutes (300)")
    parser.add_argument("--rounds", type=int, default=0, help=argparse.SUPPRESS)
    parser.add_argument("--exchange-url", default=app_token.EXCHANGE_URL, help=argparse.SUPPRESS)
    return parser.parse_args(argv)


def main(argv):
    args = parse(argv)
    request_token = "" if sys.stdin.isatty() else sys.stdin.readline().strip()
    if not request_token:
        print("token-sidecar: skipped, no request token on stdin", flush=True)
        return 0
    request_url = os.environ.get("ACTIONS_ID_TOKEN_REQUEST_URL")
    if not request_url:
        print("token-sidecar: skipped, no ACTIONS_ID_TOKEN_REQUEST_URL: the job lacks `id-token: write`", flush=True)
        return 0
    started = time.monotonic()
    deadline = started + args.max_minutes * 60
    due = started + args.interval * 60
    minted = 0
    print(f"token-sidecar: started, first mint in {args.interval:g} min, then every {args.interval:g}", flush=True)
    while True:
        if due > deadline:
            print(f"token-sidecar: done, {minted} mints, --max-minutes reached", flush=True)
            return 0
        time.sleep(max(0.0, due - time.monotonic()))
        age = f"{(time.monotonic() - started) / 60:.0f} min"
        token, detail = mint(request_url, request_token, args.exchange_url)
        if token is None:
            print(f"token-sidecar: mint failed at {age}: {detail}; next try in {args.retry:g} s", flush=True)
            due = time.monotonic() + args.retry
            continue
        try:
            write(args.path, token)
        except OSError as error:
            print(f"token-sidecar: minted at {age} but not written: {type(error).__name__}: {error}; "
                  f"next try in {args.retry:g} s", flush=True)
            due = time.monotonic() + args.retry
            continue
        minted += 1
        print(f"token-sidecar: minted {minted} at {age}: {detail}", flush=True)
        if args.rounds and minted >= args.rounds:
            print(f"token-sidecar: done, {minted} mints, --rounds reached", flush=True)
            return 0
        due = time.monotonic() + args.interval * 60


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
