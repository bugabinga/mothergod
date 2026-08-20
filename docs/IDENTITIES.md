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
| Telegram | mothergod status bot (operator-created) | operator alerts, weekly digest, operator inbox — **permanently private**, never a public channel; any public Telegram presence would be a separate BDFL-created identity | `MOTHERGOD_STATUS_BOT_TOKEN`; operator chat id in repo variable `OPERATOR_TELEGRAM_CHAT_ID` | 2026-08-20 |
