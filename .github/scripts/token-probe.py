#!/usr/bin/env python3
"""Exchange the runner's OIDC token for a claude app token twice, a gap apart, and print the statuses (#1687, step 1).

The question: does the exchange allow a second mint inside one job? Two
2xx mean a sidecar can re-mint every 45 minutes and the hour-mark token
death is removable without the operator; anything else on a later round
is plan B, an app this project owns. Each round: a fresh OIDC token from
the runner (ACTIONS_ID_TOKEN_REQUEST_URL and ACTIONS_ID_TOKEN_REQUEST_TOKEN,
which `id-token: write` exposes to every step), one POST to the exchange,
one line with both statuses, then the minted token revoked at GitHub the
way the action's own last step does, so no live credential outlives the
probe. Nothing printed carries a token: a 2xx line carries the token's
expires_at, a failure line the error code and message the exchange gave,
which the action prints too; #1687 asked for status codes alone, and the
code and message are what classify a refusal, so they ride along. A
request that got no HTTP answer prints as status 0 with its reason, never
as a traceback, and a token minted before it still reaches its revoke.

The verdict reads the rounds: `re-mint allowed` when every round minted,
`first mint failed, re-minting untested` when none did (the likeliest
cause is the workflow not matching the default branch, which says nothing
about a second mint), `re-mint refused` when a round after a mint got a
4xx, `inconclusive` when the only misses after a mint were 5xx or no
answer (the action retries those with backoff; the probe does not, so its
reader re-dispatches). A revoke that got no answer
is on its round's line, the token live until the expires_at beside it.

Usage: token-probe.py [--gap SECONDS] [--rounds N] [--exchange-url URL] [--github-api URL]

The two URL flags exist for the test; no workflow sets them. Exit 0 on
`re-mint allowed`, 1 on either other verdict, 2 when the runner exposed
no OIDC request variables, which is the missing `id-token: write`.
"""

import argparse
import os
import sys
import time

import app_token


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--gap", type=float, default=120, help="seconds between rounds")
    parser.add_argument("--rounds", type=int, default=2)
    parser.add_argument("--exchange-url", default=app_token.EXCHANGE_URL)
    parser.add_argument("--github-api", default=app_token.GITHUB_API)
    args = parser.parse_args()

    request_url = os.environ.get("ACTIONS_ID_TOKEN_REQUEST_URL")
    request_token = os.environ.get("ACTIONS_ID_TOKEN_REQUEST_TOKEN")
    if not request_url or not request_token:
        print("token-probe: no ACTIONS_ID_TOKEN_REQUEST_URL/TOKEN in the environment: the job lacks `id-token: write`")
        return 2

    minted = 0
    refused = False
    for n in range(1, args.rounds + 1):
        if n > 1:
            time.sleep(args.gap)
        when = "" if n == 1 else f" (after {args.gap:g}s)"
        status, oidc = app_token.oidc_token(request_url, request_token)
        if oidc is None:
            print(f"round {n}{when}: oidc {status}, no token in the response")
            continue
        status, token, body = app_token.exchange(oidc, url=args.exchange_url)
        if token is None:
            # Only a round after a mint says anything about re-minting: a first
            # mint refused is usually the workflow missing from the default branch.
            refused = refused or (minted > 0 and 400 <= status < 500)
            print(f"round {n}{when}: oidc 200, exchange {status}, {reason(body)}")
            continue
        minted += 1
        revoked = app_token.revoke(token, api=args.github_api)
        expires = body.get("expires_at") or "not stated"
        revoke = f"revoke {revoked}" if revoked else "revoke unreachable, token live until expires_at"
        print(f"round {n}{when}: oidc 200, exchange {status}, expires_at {expires}, {revoke}")

    if minted == args.rounds:
        verdict = "re-mint allowed"
    elif minted == 0:
        verdict = "first mint failed, re-minting untested"
    elif refused:
        verdict = "re-mint refused"
    else:
        verdict = "inconclusive"
    print(f"token-probe: rounds {args.rounds} | minted {minted} | {verdict}")
    return 0 if minted == args.rounds else 1


def reason(body):
    """`<error code>: <message>` from an exchange failure body, each part defaulted when absent."""
    error = body.get("error") if isinstance(body.get("error"), dict) else {}
    details = error.get("details") if isinstance(error.get("details"), dict) else {}
    code = details.get("error_code") or "no error code"
    message = error.get("message") or body.get("message") or "no message"
    return f"{code}: {str(message)[:200]}"


if __name__ == "__main__":
    sys.exit(main())
