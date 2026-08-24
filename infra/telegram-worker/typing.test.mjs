// The typing indicator's state machine, driven through the sequences that
// broke it. Three defects in this logic were found by hand-tracing during
// one PR review (PR #155); each one below is that trace, executable.
//
// Only `Typing.fetch` is exercised, which is the whole state machine.
// `alarm()` is a Telegram call plus a timer and has nothing to decide.
//
// Run: node --test infra/telegram-worker/*.test.mjs
// (the file glob, not the directory: node 22 tries to execute a directory
// argument as a module and dies before discovering anything in it)
import assert from "node:assert/strict";
import { test } from "node:test";
import { Typing } from "./worker.js";

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
  const typing = new Typing(state, {});
  return {
    async call(verb) {
      const response = await typing.fetch(
        new Request(`https://typing.invalid${verb}`, { method: "POST" }),
      );
      assert.equal(response.status, 204);
    },
    // The operator sees "typing..." exactly while the alarm loop is armed.
    get typing() {
      return alarm !== null;
    },
    get owed() {
      return (storage.get("arrivals") ?? []).length;
    },
  };
}

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
