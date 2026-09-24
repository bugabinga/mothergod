// The typing indicator's state machine, driven through the sequences that
// broke it. Three defects in this logic were found by hand-tracing during
// one PR review (PR #155); each one below is that trace, executable.
//
// `Typing.fetch` is the state machine. `alarm()` used to be a Telegram
// call plus a timer with nothing to decide; since the draft (#733) it
// decides whether the draft goes out this tick, so it is exercised too,
// against a fetch stub that records which Bot API method was called.
//
// Run: node --test infra/telegram-worker/*.test.mjs
// (the file glob, not the directory: node 22 tries to execute a directory
// argument as a module and dies before discovering anything in it)
import assert from "node:assert/strict";
import { test } from "node:test";
import { Typing } from "./worker.js";

const sent = [];
globalThis.fetch = async (url, init) => {
  sent.push({ method: String(url).split("/").at(-1), body: JSON.parse(init.body) });
  return new Response(null, { status: 200 });
};

// Time is a parameter here, never the wall clock: the interesting cases are
// all about ordering, and a test that races the millisecond is a test that
// fails on someone else's Tuesday.
let now = 1_700_000_000_000;
const at = (ms) => {
  now = ms;
};
Date.now = () => now;

function object() {
  const storage = new Map();
  let alarm = null;
  const state = {
    storage: {
      async get(key) {
        return storage.get(key);
      },
      async put(key, value) {
        if (typeof key === "object") {
          for (const [k, v] of Object.entries(key)) storage.set(k, v);
        } else {
          storage.set(key, value);
        }
      },
      async delete(keys) {
        for (const key of [].concat(keys)) storage.delete(key);
      },
      async deleteAll() {
        storage.clear();
      },
      async setAlarm(when) {
        alarm = when;
      },
      async deleteAlarm() {
        alarm = null;
      },
    },
  };
  sent.length = 0;
  const typing = new Typing(state, { BOT_TOKEN: "bot-token", OPERATOR_CHAT_ID: "7" });
  return {
    async call(verb) {
      const response = await typing.fetch(
        new Request(`https://typing.invalid${verb}`, { method: "POST" }),
      );
      assert.equal(response.status, 204);
    },
    // A run's draft, as tg-draft posts it; answers the object's status.
    async draft(body) {
      const response = await typing.fetch(
        new Request("https://typing.invalid/draft", { method: "POST", body }),
      );
      return response.status;
    },
    // One alarm tick, the moment the loop would send.
    tick: () => typing.alarm(),
    // The operator sees "typing..." exactly while the alarm loop is armed.
    get typing() {
      return alarm !== null;
    },
    get owed() {
      return (storage.get("arrivals") ?? []).length;
    },
    get draftText() {
      return storage.get("draft");
    },
    // Bot API methods called so far, in order.
    get methods() {
      return sent.map((call) => call.method);
    },
    get lastDraft() {
      return sent.filter((call) => call.method === "sendMessageDraft").at(-1)?.body;
    },
  };
}

const text = (line) => JSON.stringify({ text: line });

test("one message, one answer", async () => {
  const bot = object();
  at(1000);
  await bot.call("/start");
  assert.ok(bot.typing);
  at(2000);
  await bot.call("/stop");
  assert.ok(!bot.typing, "the answer landed, so nobody is waiting");
});

test("a drain keeps typing between the answers it owes", async () => {
  const bot = object();
  for (const arrival of [1000, 1100, 1200]) {
    at(arrival);
    await bot.call("/start");
  }
  at(2000);
  await bot.call("/stop");
  assert.ok(bot.typing, "two answers still owed");
  assert.equal(bot.owed, 2);
  await bot.call("/stop");
  await bot.call("/stop");
  assert.ok(!bot.typing);
});

test("a message that arrives mid-run survives that run's reset", async () => {
  const bot = object();
  const runA = 1000;
  at(1500); // the operator writes while run A is still working
  await bot.call("/start");
  at(2000);
  await bot.call(`/reset?since=${runA}`);
  assert.ok(bot.typing, "run B is queued and will answer this one");
  const runB = 2500;
  at(3000);
  await bot.call(`/reset?since=${runB}`);
  assert.ok(!bot.typing, "run B ended without answering: stop lying");
});

test("a paused or dead run stops the indicator it cannot serve", async () => {
  const bot = object();
  at(1000);
  await bot.call("/start");
  at(1500); // the guard skipped the session; the job still ends
  await bot.call(`/reset?since=1400`);
  assert.ok(!bot.typing);
});

test("a reset without a usable cutoff settles everything", async () => {
  for (const query of ["", "?since=", "?since=nonsense", "?since=-1"]) {
    const bot = object();
    at(1000);
    await bot.call("/start");
    at(2000);
    await bot.call(`/reset${query}`);
    assert.ok(!bot.typing, `silence is the safe fallback for ${query || "no query"}`);
  }
});

test("an answer nobody was owed changes nothing", async () => {
  const bot = object();
  at(1000);
  await bot.call("/stop");
  assert.ok(!bot.typing);
  assert.equal(bot.owed, 0);
});

test("a second run's reset cannot resurrect a settled indicator", async () => {
  const bot = object();
  at(1000);
  await bot.call("/start");
  at(1100);
  await bot.call("/stop");
  at(1200);
  await bot.call("/reset?since=900");
  assert.ok(!bot.typing);
});

// The draft (#733): typing with content, on the same loop, under the
// same invariant. It exists exactly while an answer is owed.

test("a draft while an answer is owed goes out on the next tick and is refreshed every 20s", async () => {
  const bot = object();
  at(1000);
  await bot.call("/start");
  assert.equal(await bot.draft(text("bdfl, 3 min\n· running the worker tests")), 204);
  at(2000);
  await bot.tick();
  assert.deepEqual(bot.methods, ["sendChatAction", "sendMessageDraft"]);
  assert.deepEqual(bot.lastDraft, {
    chat_id: "7",
    draft_id: 1,
    text: "bdfl, 3 min\n· running the worker tests",
  });
  at(6000);
  await bot.tick();
  assert.deepEqual(
    bot.methods,
    ["sendChatAction", "sendMessageDraft", "sendChatAction"],
    "inside the preview window the same draft is not re-sent",
  );
  at(22_500);
  await bot.tick();
  assert.equal(bot.methods.filter((m) => m === "sendMessageDraft").length, 2, "past 20s the preview is refreshed");
});

test("a changed draft goes out on the next tick, not on the next refresh", async () => {
  const bot = object();
  at(1000);
  await bot.call("/start");
  await bot.draft(text("a"));
  at(2000);
  await bot.tick();
  assert.equal(bot.lastDraft.text, "a");
  at(3000);
  await bot.draft(text("b"));
  at(6000);
  await bot.tick();
  assert.equal(bot.lastDraft.text, "b");
});

test("a draft with nobody waiting is dropped, and wakes nothing", async () => {
  const bot = object();
  at(1000);
  assert.equal(await bot.draft(text("bdfl, starting\n· reading the inbox")), 204);
  assert.ok(!bot.typing);
  assert.equal(bot.draftText, undefined);
  await bot.tick();
  assert.deepEqual(bot.methods, []);
});

test("a draft is proof of life: the cap counts from the run's last word", async () => {
  const bot = object();
  at(0);
  await bot.call("/start");
  at(19 * 60_000);
  await bot.draft(text("bdfl, 19 min\n· still working"));
  at(21 * 60_000);
  await bot.tick();
  assert.ok(bot.typing, "the run spoke two minutes ago");
  assert.ok(bot.methods.includes("sendMessageDraft"));
});

test("the answer landing clears the draft with the indicator", async () => {
  const bot = object();
  at(1000);
  await bot.call("/start");
  await bot.draft(text("bdfl, 1 min\n· sending the reply"));
  at(2000);
  await bot.call("/stop");
  assert.ok(!bot.typing);
  assert.equal(bot.draftText, undefined);
});

test("a run's reset drops its draft even while the next run's message waits", async () => {
  const bot = object();
  at(1000);
  await bot.call("/start");
  await bot.draft(text("bdfl, 1 min\n· pushing"));
  at(1500);
  await bot.call("/start");
  at(2000);
  await bot.call("/reset?since=1200");
  assert.ok(bot.typing, "the second message is still owed");
  assert.equal(bot.draftText, undefined, "but a dead run's last words are not shown for it");
});

test("a malformed draft is refused and stores nothing", async () => {
  const bot = object();
  at(1000);
  await bot.call("/start");
  for (
    const body of ["not json", "null", "{}", text(""), text("   "), JSON.stringify({ text: 5 }), text("x".repeat(4097))]
  ) {
    assert.equal(await bot.draft(body), 400, body.slice(0, 20));
  }
  assert.equal(bot.draftText, undefined);
});
