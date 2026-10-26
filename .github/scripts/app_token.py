#!/usr/bin/env python3
"""Mint a claude app token the way the pinned claude-code-action does, as functions a sidecar can call (#1687).

The action mints one token at the start of a job and GitHub expires it an
hour later, inside a job that may run four (GOVERNANCE "Token lifetime").
Each function here copies one step of the action's src/github/token.ts at
the pin every workflow names (`grep -ho 'claude-code-action@[0-9a-f]*'
.github/workflows/*.yml | sort -u`): the OIDC token is requested from the
runner for the audience the action asks for, exchanged at the action's
URL, and revoked at GitHub's the way the action's last step does. A pin
bump that moves either constant moves this file; token-probe.py's first
exchange status is the check.

Never prints or logs a token: every function returns it and the caller
decides where it goes. A status code is the only thing a caller should
print; a failure body carries no credential and may be shown.
"""

import json
import urllib.error
import urllib.parse
import urllib.request

AUDIENCE = "claude-code-github-action"
EXCHANGE_URL = "https://api.anthropic.com/api/github/github-app-token-exchange"
GITHUB_API = "https://api.github.com"
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
    """(status, body dict) for a request, an HTTP error answered the same way."""
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            return resp.status, _body(resp)
    except urllib.error.HTTPError as err:
        return err.code, _body(err)


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
    code and message, which carry no credential.
    """
    req = urllib.request.Request(
        url,
        method="POST",
        headers={"Authorization": f"Bearer {oidc}", "Accept": "application/json"},
    )
    status, body = _send(req)
    token = (body.get("token") or body.get("app_token")) if 200 <= status < 300 else None
    return status, token or None, body


def revoke(app_token, api=GITHUB_API):
    """The status of DELETE /installation/token: what the action's last step does with a token it minted."""
    req = urllib.request.Request(
        f"{api}/installation/token",
        method="DELETE",
        headers={"Authorization": f"Bearer {app_token}", "Accept": "application/vnd.github+json"},
    )
    status, _ = _send(req)
    return status
