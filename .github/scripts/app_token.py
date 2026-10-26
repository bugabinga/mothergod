#!/usr/bin/env python3
"""The claude app token beside a session: minted the way the pinned claude-code-action mints it, kept in a file the writers read (#1687).

The action mints one token at the start of a job and GitHub expires it
an hour later, inside a job that may run four (GOVERNANCE "Token
lifetime"). token-sidecar.py re-mints with the two functions below and
writes the newest to the file GH_TOKEN_FILE names; `prefer_file` is the
reader, called by gh-pr, push-branch, merge-pr and settle-push at each
gh call. Each minting function copies one step of the action at the pin
every workflow names (`grep -ho 'claude-code-action@[0-9a-f]*'
.github/workflows/*.yml | sort -u`): the OIDC token is requested from
the runner for the audience src/github/token.ts asks for and exchanged
at the URL it POSTs to. A pin bump that moves either constant moves this
file; the sidecar's first exchange status in any seat's log is the check.

Never prints or logs a token: every function returns it and the caller
decides where it goes. A status code is the only thing a caller should
print; a failure body carries no credential and may be shown. A request
that got no HTTP answer at all (refused connection, DNS, timeout, a body
cut short) is status 0 with the reason in the body's error message,
never an exception: the caller is a sidecar whose next line must still
print.
"""

import http.client
import json
import urllib.error
import urllib.parse
import urllib.request

AUDIENCE = "claude-code-github-action"
EXCHANGE_URL = "https://api.anthropic.com/api/github/github-app-token-exchange"
TIMEOUT = 30


def _body(resp):
    """The response body as a dict; {} for an empty or non-JSON body."""
    raw = resp.read()
    try:
        parsed = json.loads(raw) if raw else {}
    except ValueError:
        return {}
    return parsed if isinstance(parsed, dict) else {}


def _send(req):
    """(status, body dict) for a request; an HTTP error the same way, no HTTP answer as status 0."""
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            return resp.status, _body(resp)
    except urllib.error.HTTPError as err:
        return err.code, _body(err)
    except (OSError, http.client.HTTPException) as err:
        # URLError (a refused connection, DNS) and TimeoutError are OSError; a
        # body cut short (IncompleteRead) or a malformed status line is not.
        reason = getattr(err, "reason", None) or err
        return 0, {"error": {"message": f"{type(err).__name__}: {reason}"}}


def oidc_token(request_url, request_token, audience=AUDIENCE):
    """(status, token or None): the runner's OIDC token for `audience`, as @actions/core getIDToken fetches it."""
    sep = "&" if "?" in request_url else "?"
    url = f"{request_url}{sep}audience={urllib.parse.quote(audience, safe='')}"
    req = urllib.request.Request(
        url,
        headers={"Authorization": f"bearer {request_token}", "Accept": "application/json"},
    )
    status, body = _send(req)
    return status, body.get("value") or None


def exchange(oidc, url=EXCHANGE_URL):
    """(status, token or None, body dict): one POST of the OIDC token to the exchange.

    The token is body['token'] or body['app_token'] on a 2xx, as the action
    reads it; the body is returned so a caller can print a failure's error
    code and message, which carry no credential, and a mint's expires_at.
    """
    req = urllib.request.Request(
        url,
        method="POST",
        headers={"Authorization": f"Bearer {oidc}", "Accept": "application/json"},
    )
    status, body = _send(req)
    token = (body.get("token") or body.get("app_token")) if 200 <= status < 300 else None
    return status, token or None, body


def reason(body):
    """`<error code>: <message>` from an exchange failure body, each part defaulted when absent."""
    error = body.get("error") if isinstance(body.get("error"), dict) else {}
    details = error.get("details") if isinstance(error.get("details"), dict) else {}
    code = details.get("error_code") or "no error code"
    message = error.get("message") or body.get("message") or "no message"
    return f"{code}: {str(message)[:200]}"


def prefer_file(environ):
    """Point environ's GH_TOKEN at the sidecar's newest mint and return it; None, environ untouched, when GH_TOKEN_FILE is unset, unreadable or empty.

    The file wins over the GH_TOKEN the session started with because it is
    never older: the sidecar's first mint comes after the action's, and
    every later one replaces it. A token a caller passes beside the
    environment on purpose (the admin PAT for a workflow path, the
    workflow token for a comment) is not this function's to change; the
    writers apply it only where the session's own token would ride.
    """
    path = environ.get("GH_TOKEN_FILE")
    if not path:
        return None
    try:
        with open(path, encoding="utf-8") as f:
            token = f.read().strip()
    except OSError:
        return None
    if not token:
        return None
    environ["GH_TOKEN"] = token
    return token
