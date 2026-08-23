// Telegram webhook -> BDFL wake (issue #5). The bot's webhook points
// here; deploy-telegram-worker.yml sets it. Four duties, nothing else:
// authenticate Telegram, store the operator's update in KV (twice: the
// inbox to work through, the chat log to remember), show the
// "typing..." indicator until an answer goes out, fire a
// workflow_dispatch so the BDFL reads it within seconds. Heavy work
// never happens here; the BDFL run is the brain, this is the doorbell.

// The chat log: one KV key holding the last KEEP turns of operator
// conversation, the only memory that outlives a run. Both sides are
// written by whoever knows the message happened, the operator's here and
// the bot's in .github/scripts/tg-send, and neither by an agent by hand,
// because bookkeeping an agent has to remember is bookkeeping it will
// eventually forget (#183).
const CHATLOG = "chatlog";
const KEEP = 40;
// Chars of an entry's text: 40 entries have to stay readable in one
// screenful, and the full update is in the inbox until the run drains it.
const SUMMARY = 200;

// sendChatAction's status expires after 5s, so the refresh sits just
// inside that.
const TYPING_TICK_MS = 4000;
// A run that dies without answering must not leave the operator watching
// a bot type forever. Past this, the indicator gives up; the reply, when
// it comes, arrives on a quiet screen instead of a lying one.
const TYPING_CAP_MS = 20 * 60 * 1000;

/**
 * The "typing..." indicator, as a self-refreshing alarm loop.
 *
 * It lives here rather than in the answering GitHub Actions run because
 * the run is structurally unable to get it right (operator report,
 * 2026-08-23, third attempt): the run starts seconds to minutes after
 * the message it answers, so it cannot type during the wait that matters
 * most, and when the lane is busy the run that types is not the run that
 * answers. The worker is awake at the exact moment the operator's
 * message arrives, which is the moment the indicator has to start.
 *
 * One instance for one operator chat, so what it tracks is not on/off but
 * `arrivals`: the wall-clock time of every message that landed and has
 * not been answered, oldest first. A drain loop answering three queued
 * messages must keep typing between replies one and three, which a
 * boolean cannot express (PR #155 review).
 *
 * Three verbs:
 * - /start, a message landed: append its arrival time.
 * - /stop, an answer went out (`.github/scripts/tg-send`): drop the
 *   oldest, because a drain answers in arrival order.
 * - /reset?since=<ms>, a run ended (agent-bdfl.yml's last step): drop
 *   every arrival older than that run's start. Those were its messages
 *   to answer, and whether it answered them, died, or was skipped by the
 *   pause guard, nobody is working on them now.
 *
 * The `since` cutoff is the whole reason /reset is not a blunt zero. A
 * message that lands mid-run belongs to the NEXT run, which the
 * concurrency lane has already queued (agent-bdfl.yml), and zeroing it
 * would leave the operator staring at a silent screen for exactly the
 * wait this indicator exists to fill (PR #155 review, third round).
 *
 * Nothing here is trusted for long: every run's /reset clears everything
 * that predates it, and the deadline clears the lot if a run dies
 * without resetting. Drift lives one run at most.
 */
export class Typing {
  constructor(state, env) {
    this.state = state;
    this.env = env;
  }

  async fetch(request) {
    const url = new URL(request.url);
    const arrivals = (await this.state.storage.get("arrivals")) ?? [];
    if (url.pathname === "/start") {
      arrivals.push(Date.now());
      await this.state.storage.put({
        arrivals,
        deadline: Date.now() + TYPING_CAP_MS,
      });
      await this.state.storage.setAlarm(Date.now());
      return new Response(null, { status: 204 });
    }
    // A missing or malformed `since` falls back to "now", which settles
    // everything: the safe direction is silence, never a bot that types
    // at nobody.
    const since = Number(url.searchParams.get("since"));
    const cutoff = since > 0 ? since : Date.now();
    const left =
      url.pathname === "/stop"
        ? arrivals.slice(1)
        : arrivals.filter((at) => at >= cutoff);
    if (left.length) {
      await this.state.storage.put("arrivals", left);
    } else {
      await this.state.storage.deleteAlarm();
      await this.state.storage.deleteAll();
    }
    return new Response(null, { status: 204 });
  }

  async alarm() {
    const deadline = await this.state.storage.get("deadline");
    // No deadline means a stop won the race with a scheduled tick. Both
    // exits leave no alarm, so the loop ends by not rescheduling.
    if (!deadline || Date.now() >= deadline) {
      await this.state.storage.deleteAll();
      return;
    }
    await fetch(
      `https://api.telegram.org/bot${this.env.BOT_TOKEN}/sendChatAction`,
      {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          chat_id: this.env.OPERATOR_CHAT_ID,
          action: "typing",
        }),
      },
    ).catch(() => {});
    await this.state.storage.setAlarm(Date.now() + TYPING_TICK_MS);
  }
}

// Never throws: a failed indicator loses decoration, never data, so it
// must not abort the webhook that carries the message. It does report,
// because the caller that ends the indicator is also our only probe
// that the object is reachable at all.
async function typing(env, verb) {
  try {
    await env.TYPING.get(env.TYPING.idFromName("operator")).fetch(
      `https://typing.invalid${verb}`,
    );
    return true;
  } catch (error) {
    console.error("typing", verb, error);
    return false;
  }
}

/**
 * Append the operator's side of the conversation to the chat log.
 *
 * Read-modify-write on one key with exactly two writers, this and
 * tg-send. They collide only if a message lands in the same moment a
 * reply goes out, and the loss is one line of memory, never an update:
 * the update is already under its own `u:` key. A third writer would
 * need real serialization, so do not add one.
 *
 * Never throws, for the same reason the reaction does not: this is
 * memory, and losing memory must not cost the message it is about.
 */
export async function remember(env, message) {
  try {
    const entries = parse(await env.INBOX.get(CHATLOG));
    entries.push({
      from: "operator",
      message_id: message.message_id,
      date: message.date,
      text: (message.text ?? "").slice(0, SUMMARY),
    });
    await env.INBOX.put(CHATLOG, JSON.stringify(entries.slice(-KEEP)));
  } catch (error) {
    console.error("chatlog", error);
  }
}

// A missing key and a corrupt one mean the same thing here: start a log.
// Parsing sits outside `remember`'s catch on purpose, so that a value
// somebody once hand-wrote to this key cannot be mistaken for KV being
// down and stop the log forever.
function parse(stored) {
  try {
    const log = JSON.parse(stored ?? "[]");
    return Array.isArray(log) ? log : [];
  } catch {
    return [];
  }
}

export default {
  async fetch(request, env) {
    if (request.method !== "POST") {
      return new Response("mothergod telegram webhook", { status: 200 });
    }
    // `/typing/stop`: one answer went out (tg-send). `/typing/reset`: a
    // run ended and owes nothing (agent-bdfl.yml's last step), which is
    // also the health probe that the object is reachable at all.
    // Authenticated with WEBHOOK_SECRET under a header of its own,
    // because the secret's role is "may talk to this worker" and both
    // callers are ours. Telegram's header name is Telegram's; ours says
    // who we are.
    const { pathname, search } = new URL(request.url);
    if (pathname === "/typing/stop" || pathname === "/typing/reset") {
      if (request.headers.get("x-mothergod-secret") !== env.WEBHOOK_SECRET) {
        return new Response(null, { status: 401 });
      }
      const ok = await typing(env, pathname.replace("/typing", "") + search);
      return new Response(null, { status: ok ? 204 : 502 });
    }
    // Telegram echoes the secret_token from setWebhook in this header;
    // a request without it is not Telegram.
    const secret = request.headers.get("x-telegram-bot-api-secret-token");
    if (secret !== env.WEBHOOK_SECRET) {
      return new Response(null, { status: 401 });
    }
    let update;
    try {
      update = await request.json();
    } catch {
      return new Response(null, { status: 200 });
    }
    const chat = update?.message?.chat?.id;
    // Non-operator chats: no action, no reply (issue #5 acceptance).
    if (
      String(chat) !== env.OPERATOR_CHAT_ID ||
      typeof update.update_id !== "number"
    ) {
      return new Response(null, { status: 200 });
    }
    // Zero-padded key so KV's lexicographic list order is arrival order.
    const key = "u:" + String(update.update_id).padStart(12, "0");
    await env.INBOX.put(key, JSON.stringify(update));
    // Instant receipt (operator request, 2026-08-23): the 👀 reaction
    // says "stored in KV, cannot be lost", so it fires right after the
    // KV write, before the GitHub dispatch, whose API latency must not
    // delay it (PR #99 review). The BDFL switches it to ✍ on pickup;
    // its reply is the completion. Non-fatal: a failed reaction loses
    // decoration, never data.
    if (update.message.message_id) {
      await fetch(
        `https://api.telegram.org/bot${env.BOT_TOKEN}/setMessageReaction`,
        {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({
            chat_id: update.message.chat.id,
            message_id: update.message.message_id,
            reaction: [{ type: "emoji", emoji: "👀" }],
          }),
        },
      ).catch(() => {});
    }
    // Eyes, then typing: the operator asked for the indicator to start
    // the moment the message is acknowledged.
    await typing(env, "/start");
    // Remembered before the wake, not after: the run that is about to
    // read this log should find it complete. Placed after the reaction
    // and the indicator, which the operator is watching for, and before
    // the dispatch, which nobody is.
    await remember(env, update.message);
    // Wake the BDFL. The dispatch carries no message text (issue #36
    // principle): it says "wake up", the run reads KV for the prose.
    // Dispatch failure is tolerable: the update is already in KV and
    // the hourly schedule (ADR-0015) is the backstop.
    const res = await fetch(
      `https://api.github.com/repos/${env.GITHUB_REPO}/actions/workflows/agent-bdfl.yml/dispatches`,
      {
        method: "POST",
        headers: {
          authorization: `Bearer ${env.GITHUB_PAT}`,
          accept: "application/vnd.github+json",
          "user-agent": "mothergod-telegram-worker",
          "content-type": "application/json",
        },
        body: JSON.stringify({ ref: "main" }),
      },
    );
    if (!res.ok) {
      console.error("dispatch failed", res.status, await res.text());
    }
    return new Response(null, { status: 200 });
  },
};
