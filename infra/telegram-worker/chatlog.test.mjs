// The chat log is memory that outlives a run, so its failures are quiet
// by nature: nothing goes red, a later run just knows less than it should.
// These are the four ways it can lose an entry, executable.
//
// Only `remember` is exercised. The call site is one line in the webhook
// handler and has nothing to decide.
//
// Run: node --test infra/telegram-worker/*.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { remember } from "./worker.js";

// KV, reduced to what `remember` uses. `get` returns the stored string or
// null, which is what a missing key gives you.
function inbox(initial = null) {
  let value = initial;
  return {
    INBOX: {
      async get() {
        return value;
      },
      async put(_key, written) {
        value = written;
      },
    },
    read: () => JSON.parse(value),
  };
}

const message = (id, text = "hello") => ({
  message_id: id,
  date: 1_700_000_000 + id,
  text,
});

test("the first message ever sent finds no key and starts the log", async () => {
  const env = inbox();
  await remember(env, message(1));
  assert.deepEqual(env.read(), [
    { from: "operator", message_id: 1, date: 1_700_000_001, text: "hello" },
  ]);
});

test("retention keeps the newest 40, not the first 40", async () => {
  const env = inbox();
  for (let id = 1; id <= 45; id++) await remember(env, message(id));
  const log = env.read();
  assert.equal(log.length, 40);
  assert.equal(log[0].message_id, 6);
  assert.equal(log.at(-1).message_id, 45);
});

test("a corrupt log is replaced, not a reason to stop remembering", async () => {
  // Both shapes a hand-written key has actually taken: unparseable, and
  // valid JSON that is not an array.
  for (const corrupt of ["not json at all", '{"from":"operator"}']) {
    const env = inbox(corrupt);
    await remember(env, message(7));
    assert.equal(env.read().length, 1);
  }
});

test("a message with no text is still a turn in the conversation", async () => {
  // A sticker or a photo: the operator said something, and a log that
  // drops it reads as a gap the next run cannot explain.
  const env = inbox();
  await remember(env, { message_id: 2, date: 1, text: undefined });
  assert.equal(env.read()[0].text, "");
});

test("a long message is summarized, because 40 of them share one screen", async () => {
  const env = inbox();
  await remember(env, message(3, "x".repeat(500)));
  assert.equal(env.read()[0].text.length, 200);
});

test("KV failing loses the memory, never the message", async () => {
  // The update is already stored under its own `u:` key when this runs.
  // Throwing here would fail the webhook and cost Telegram's delivery.
  const env = {
    INBOX: {
      async get() {
        throw new Error("KV unavailable");
      },
      async put() {},
    },
  };
  await assert.doesNotReject(() => remember(env, message(4)));
});
