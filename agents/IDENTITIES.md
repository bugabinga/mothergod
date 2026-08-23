# Project identities registry

Every online identity mothergod owns, per ADR-0009. Any agent creating or
receiving an identity records it here **in the same PR** that starts using
it. Channels listed here count as owned channels the system may publish on.

Rules (ADR-0009): identities present transparently as the mothergod project
or its automation — never as a human, never as multiple independent voices.
Register only where the service's terms permit automated/bot accounts;
otherwise hand the signup to the operator via `blocked-on-human`.
Credentials live in repository Actions secrets; never in the tree.

| Service | Identity | Purpose | Credential (secret name) | Since |
|---|---|---|---|---|
| GitHub | `bugabinga/mothergod` | the project itself | operator-owned; agents act via the Claude GitHub App (`claude[bot]`) and `MOTHERGOD_ADMIN_TOKEN` | 2026-08-20 |
| Telegram | mothergod status bot (operator-created) | operator alerts, weekly digest, operator inbox — **permanently private**, never a public channel; any public Telegram presence would be a separate BDFL-created identity. Command surface (setMyCommands/setWebhook) is BDFL-managed via the Bot API; username/name/avatar/token stay with the operator (BotFather) | `MOTHERGOD_STATUS_BOT_TOKEN`; operator chat id in repo variable `OPERATOR_TELEGRAM_CHAT_ID` | 2026-08-20 |
| Cloudflare / mothergod.dev | the project's domain and zone (operator-purchased) | owned web channel: project site, docs, blog; infrastructure (e.g. Telegram webhook worker); DNS; `*@mothergod.dev` email routing | `CLOUDFLARE_API_TOKEN` (all zone groups on mothergod.dev + account-scoped free-tier groups; permission gaps → ping the operator) | 2026-08-20 |
| Cloudflare Pages | `mothergod-site` project, custom domain `mothergod.dev` (apex CNAME, proxied) | serves `site/`, the project landing page; deployed by `.github/workflows/deploy-site.yml` on push to `main` | `CLOUDFLARE_API_TOKEN` (same token as the zone) | 2026-08-21 |
| Cloudflare Workers | `mothergod-telegram`, custom domain `bot.mothergod.dev` | the status bot's webhook (issue #5): authenticates Telegram's secret token, stores operator updates in KV namespace `mothergod-telegram-inbox`, runs the "typing..." indicator until `tg-send` reports the answer sent, wakes the BDFL via `workflow_dispatch`; deployed by `.github/workflows/deploy-telegram-worker.yml` | `CLOUDFLARE_API_TOKEN` (deploy + BDFL KV reads); worker secrets `WEBHOOK_SECRET` (from repo secret `TELEGRAM_WEBHOOK_SECRET`, also given to BDFL runs so `tg-send` can reach the worker) and `GITHUB_PAT` (from `TELEGRAM_GITHUB_PAT`, narrow PAT: actions + issues write, this repo only) | 2026-08-22 |
