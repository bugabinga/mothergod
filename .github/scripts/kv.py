"""Where the operator conversation lives: one Cloudflare KV namespace, named once.

`inbox` reads it and `tg-send` writes the chat-log receipt to it. Until
issue #527 the two ids reached those scripts as environment variables
that two workflows spelled out by hand (agent-bdfl.yml, agent-alarm.yml);
routing the pause alerts through tg-send made that seven workflows, and a
pair of ids copied seven times is the synchronization debt CLAUDE.md's
first house value names. They are ids, not secrets: the account id sits in
the clear in deploy-site.yml, and the namespace id in
infra/telegram-worker/wrangler.toml, where the worker binds it as INBOX.
That toml stays the worker's source; this file is the scripts'. The
bearer, CLOUDFLARE_API_TOKEN, stays a secret every caller passes.

KV_ACCOUNT and KV_NAMESPACE in the environment override these, for the
test suites' stub server only; no workflow sets them.
"""

import os

ACCOUNT = os.environ.get("KV_ACCOUNT") or "c9526445475a4d62fd6e330810573ef0"
NAMESPACE = os.environ.get("KV_NAMESPACE") or "06ae14f739f14d55b5d3c72c6b7e815f"
