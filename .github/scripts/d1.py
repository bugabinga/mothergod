"""Where the session table lives: one Cloudflare D1 database, named once (ADR-0060).

`session-row.py` writes it and `sessions` reads it, the same split kv.py
draws for the operator conversation. The database id is an identifier, not a
secret, like the KV namespace id beside it; the bearer, CLOUDFLARE_API_TOKEN,
stays a secret every caller passes. The schema is infra/sessions/schema.sql.

D1_API_BASE and D1_DATABASE in the environment override the endpoint and id
for the test suites' stub server only; no workflow sets them.
"""

import json
import os
import urllib.error
import urllib.request

import kv

DATABASE = os.environ.get("D1_DATABASE") or "df8eba02-caf2-45ad-9c56-bcb1e1ebbc31"
URL = (os.environ.get("D1_API_BASE", "https://api.cloudflare.com")
       + f"/client/v4/accounts/{kv.ACCOUNT}/d1/database/{DATABASE}/query")
# Cloudflare's edge answers 403 to urllib's default User-Agent (tg-send's
# lesson); our tooling says who it is.
UA = "mothergod-sessions"


class Unreadable(Exception):
    """The database could not be read or written; the message is token-free."""


def query(sql, params=(), token=None):
    """Run `sql` and return its rows as dicts.

    One statement with `params`, or a `;`-separated batch with none: the
    endpoint accepts both and returns one result per statement, which this
    flattens. Any failure, transport, HTTP or a `success: false` payload,
    raises Unreadable with the token scrubbed out of the text (hard rule 10),
    so a caller cannot mistake a dead endpoint for an empty table.
    """
    token = token or os.environ.get("CLOUDFLARE_API_TOKEN", "")
    if not token:
        raise Unreadable("CLOUDFLARE_API_TOKEN is unset")
    body = json.dumps({"sql": sql, "params": list(params)}).encode()
    request = urllib.request.Request(
        URL,
        data=body,
        method="POST",
        headers={"Authorization": f"Bearer {token}",
                 "User-Agent": UA,
                 "Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            payload = json.load(response)
    except urllib.error.HTTPError as error:
        detail = error.read().decode("utf-8", "replace")[:400]
        raise Unreadable(f"HTTP {error.code}: {detail}".replace(token, "<redacted>")) from None
    except (OSError, ValueError) as error:
        raise Unreadable(str(error).replace(token, "<redacted>")) from None
    if not isinstance(payload, dict) or not payload.get("success"):
        errors = payload.get("errors", []) if isinstance(payload, dict) else []
        reason = "; ".join(str(e.get("message", "?")) for e in errors if isinstance(e, dict))
        raise Unreadable((reason or "success=false").replace(token, "<redacted>"))
    rows = []
    for statement in payload.get("result") or []:
        if isinstance(statement, dict):
            rows.extend(r for r in statement.get("results") or [] if isinstance(r, dict))
    return rows
