// Telegram webhook -> BDFL wake (issue #5). The bot's webhook points
// here; deploy-telegram-worker.yml sets it. Four duties, nothing else:
// authenticate Telegram, store the operator's update in KV, show the
// "typing..." indicator until an answer goes out, fire a
// workflow_dispatch so the BDFL reads it within seconds. Heavy work
// never happens here; the BDFL run is the brain, this is the doorbell.

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
 * `owed`: messages that arrived and have not been answered yet. A drain
 * loop answering three queued messages must keep typing between replies
 * one and three, which a boolean cannot express (PR #155 review).
 *
 * Three verbs: /start when a message lands (owed + 1), /stop when an
 * answer goes out (owed - 1, `.github/scripts/tg-send`), /reset when the
 * run ends owing nothing (agent-bdfl.yml's last step). The count is an
 * estimate — a run may fold two answers into one message, or send an
 * unprompted digest — so it is never trusted for long: /reset zeroes it
 * at the end of every run, and the deadline zeroes it if a run dies
 * without resetting. Drift lives one run at most.
 */
export class Typing {
  constructor(state, env) {
    this.state = state;
    this.env = env;
  }

  async fetch(request) {
    const { pathname } = new URL(request.url);
    const owed = (await this.state.storage.get("owed")) ?? 0;
    if (pathname === "/start") {
      await this.state.storage.put({
        owed: owed + 1,
        deadline: Date.now() + TYPING_CAP_MS,
      });
      await this.state.storage.setAlarm(Date.now());
      return new Response(null, { status: 204 });
    }
    const left = pathname === "/stop" ? Math.max(0, owed - 1) : 0;
    if (left > 0) {
      await this.state.storage.put("owed", left);
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
    const path = new URL(request.url).pathname;
    if (path === "/typing/stop" || path === "/typing/reset") {
      if (request.headers.get("x-mothergod-secret") !== env.WEBHOOK_SECRET) {
        return new Response(null, { status: 401 });
      }
      const ok = await typing(env, path.replace("/typing", ""));
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
