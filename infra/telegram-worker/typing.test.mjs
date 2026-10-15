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
      async getAlarm() {
        return alarm;
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
    // One alarm tick, the moment the loop would send. An alarm is consumed
    // by firing, so a handler that does not reschedule ends the loop.
    tick: () => {
      alarm = null;
      return typing.alarm();
    },
    // The operator sees "typing..." exactly while the alarm loop is armed.
    get typing() {
      return alarm !== null;
    },
    get owed() {
      return (storage.get("arrivals") ?? []).length;
    },
    // The live runs' texts, oldest run first: what the next tick composes.
    get drafts() {
      return (storage.get("runs") ?? []).map((entry) => entry.text);
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

const text = (line, run = "r1") => JSON.stringify({ text: line, run });

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

// The draft (#733): typing with content, on the same loop. It exists
// while an answer is owed or a run is working (#742), per run.

test("a draft while an answer is owed goes out on the next tick and is refreshed every 10s", async () => {
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
  at(12_500);
  await bot.tick();
  assert.equal(bot.methods.filter((m) => m === "sendMessageDraft").length, 2, "past 10s the preview is refreshed");
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

test("a scheduled run's draft shows with nobody waiting, and its first word starts the loop", async () => {
  const bot = object();
  at(1000);
  assert.equal(await bot.draft(text("maintainer, starting\n· reading the queue", "m1")), 204);
  assert.ok(bot.typing, "no /start preceded it: the draft armed the loop itself");
  at(2000);
  await bot.tick();
  assert.deepEqual(bot.methods, ["sendChatAction", "sendMessageDraft"]);
  assert.equal(bot.lastDraft.text, "maintainer, starting\n· reading the queue");
});

test("two live runs share the draft, oldest run first, each keeping its slot", async () => {
  const bot = object();
  at(1000);
  await bot.draft(text("maintainer, 1 min\n· building", "m1"));
  at(1500);
  await bot.draft(text("reviewer, starting\n· reading the diff", "r2"));
  at(2000);
  await bot.draft(text("maintainer, 1 min\n· testing", "m1"));
  at(3000);
  await bot.tick();
  assert.equal(
    bot.lastDraft.text,
    "maintainer, 1 min\n· testing\n\nreviewer, starting\n· reading the diff",
  );
});

test("a run's reset drops its text and leaves the other run's", async () => {
  const bot = object();
  at(1000);
  await bot.draft(text("maintainer\n· building", "m1"));
  await bot.draft(text("reviewer\n· reading", "r2"));
  at(2000);
  await bot.call("/reset?since=900&run=m1");
  assert.ok(bot.typing, "the reviewer is still working");
  assert.deepEqual(bot.drafts, ["reviewer\n· reading"]);
  await bot.call("/reset?since=900&run=r2");
  assert.ok(!bot.typing, "nobody working, nothing owed: the loop ends");
});

test("a run that falls silent for the cap drops out while another keeps the loop", async () => {
  const bot = object();
  at(0);
  await bot.draft(text("reviewer\n· last words before the cancel", "r2"));
  at(15 * 60_000);
  await bot.draft(text("maintainer\n· still going", "m1"));
  at(21 * 60_000);
  await bot.tick();
  assert.ok(bot.typing);
  assert.equal(bot.lastDraft.text, "maintainer\n· still going", "the cancelled review's words are gone");
  at(36 * 60_000);
  await bot.tick();
  assert.ok(!bot.typing, "the last run went quiet past the cap");
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

test("the answer landing clears what was owed; the run's draft stays until its reset", async () => {
  const bot = object();
  at(1000);
  await bot.call("/start");
  await bot.draft(text("bdfl, 1 min\n· sending the reply"));
  at(2000);
  await bot.call("/stop");
  assert.equal(bot.owed, 0);
  assert.ok(bot.typing, "the run goes on after the reply, and the operator asked to watch it");
  assert.deepEqual(bot.drafts, ["bdfl, 1 min\n· sending the reply"]);
  await bot.call("/reset?since=900&run=r1");
  assert.ok(!bot.typing);
});

test("a run's reset drops its draft even while the next run's message waits", async () => {
  const bot = object();
  at(1000);
  await bot.call("/start");
  await bot.draft(text("bdfl, 1 min\n· pushing"));
  at(1500);
  await bot.call("/start");
  at(2000);
  await bot.call("/reset?since=1200&run=r1");
  assert.ok(bot.typing, "the second message is still owed");
  assert.deepEqual(bot.drafts, [], "but a dead run's last words are not shown for it");
});

test("a reset naming no run drops every run's text", async () => {
  const bot = object();
  at(1000);
  await bot.draft(text("a", "r1"));
  await bot.draft(text("b", "r2"));
  await bot.call("/reset?since=900");
  assert.ok(!bot.typing);
  assert.deepEqual(bot.drafts, []);
});

test("a malformed draft is refused and stores nothing", async () => {
  const bot = object();
  at(1000);
  await bot.call("/start");
  for (
    const body of [
      "not json",
      "null",
      "{}",
      text(""),
      text("   "),
      JSON.stringify({ text: 5, run: "r1" }),
      text("x".repeat(4097)),
      JSON.stringify({ text: "x" }),
      text("x", ""),
      text("x", "a b"),
      text("x", "r".repeat(65)),
      JSON.stringify({ text: "x", run: 5 }),
    ]
  ) {
    assert.equal(await bot.draft(body), 400, body.slice(0, 20));
  }
  assert.deepEqual(bot.drafts, []);
});
