// tg-send's stdout is a contract a session reads mid-run and acts on. A bare
// integer there was read as HTTP 429 once, retried three times, and posted
// the operator four identical status lines (#535): success and failure must
// not look alike. A local server stands in for Telegram over
// TELEGRAM_API_BASE, and these pin what each exit prints.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { after, test } from "node:test";

const script = new URL("tg-send", import.meta.url).pathname;

// The token segment of `/bot<token>/sendMessage` selects the reply, so a test
// picks its outcome by picking its token, and the token doubles as the secret
// that must never reach stderr.
const REPLIES = {
  "tok-ok": [200, { ok: true, result: { message_id: 434, date: 1757700000 } }],
  "tok-denied": [400, { ok: false, error_code: 400, description: "Bad Request: chat not found" }],
};
const seen = [];
const server = createServer((req, res) => {
  const token = req.url.split("/")[1].slice("bot".length);
  const [status, reply] = REPLIES[token] ?? [404, { ok: false, description: "Not Found" }];
  let data = "";
  req.on("data", (chunk) => (data += chunk));
  req.on("end", () => {
    seen.push(JSON.parse(data));
    res.writeHead(status, { "content-type": "application/json" });
    res.end(JSON.stringify(reply));
  });
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const base = `http://127.0.0.1:${server.address().port}`;
after(() => server.close());

// spawn, never spawnSync: the stub answers on this same event loop, and a
// synchronous child would hold it until the client's 20-second timeout.
function send(token, { body = "wake: schedule, no-op", args = [], api = base } = {}) {
  return new Promise((resolve) => {
    const child = spawn("python3", [script, ...args], {
      // No Cloudflare or worker credentials on purpose: the chat-log receipt
      // and the typing decrement both degrade to a stderr note, and neither
      // is what these assert.
      env: {
        PATH: process.env.PATH,
        TELEGRAM_API_BASE: api,
        TELEGRAM_BOT_TOKEN: token,
        OPERATOR_TELEGRAM_CHAT_ID: "42",
      },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8").on("data", (chunk) => (stdout += chunk));
    child.stderr.setEncoding("utf8").on("data", (chunk) => (stderr += chunk));
    child.on("close", (status) => resolve({ status, stdout, stderr }));
    child.stdin.end(body);
  });
}

test("a delivered message prints message_id=<n> and nothing else on stdout", async () => {
  const { status, stdout } = await send("tok-ok");
  assert.equal(status, 0);
  assert.equal(stdout, "message_id=434\n");
});

test("a Telegram refusal exits 1 with an empty stdout and the API's reason on stderr", async () => {
  const { status, stdout, stderr } = await send("tok-denied");
  assert.equal(status, 1);
  assert.equal(stdout, "");
  assert.match(stderr, /tg-send: HTTP 400: Bad Request: chat not found/);
  assert.doesNotMatch(stderr, /tok-denied/);
});

test("an unreachable API exits 1 and the token is scrubbed from the URL in the error", async () => {
  const { status, stdout, stderr } = await send("tok-secret", { api: "http://127.0.0.1:1" });
  assert.equal(status, 1);
  assert.equal(stdout, "");
  assert.match(stderr, /^tg-send: /);
  assert.doesNotMatch(stderr, /tok-secret/);
});

test("refs become issue links and --reply-to rides as reply_parameters", async () => {
  const { status } = await send("tok-ok", { body: "shipped #535", args: ["--reply-to", "7"] });
  assert.equal(status, 0);
  const payload = seen.at(-1);
  assert.equal(payload.chat_id, "42");
  assert.deepEqual(payload.reply_parameters, { message_id: 7 });
  assert.equal(
    payload.text,
    "shipped <a href=\"https://github.com/bugabinga/mothergod/issues/535\">#535</a>",
  );
});
