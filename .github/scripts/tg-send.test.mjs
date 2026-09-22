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
  // A body that echoes the request path is how a token reaches stderr in
  // practice (an edge or proxy error page, never Telegram's own JSON); the
  // scrub is all that stands between that and hard rule 10.
  const [status, reply] = token === "tok-echo"
    ? [502, { ok: false, description: `Bad Gateway: no upstream for ${req.url}` }]
    : (REPLIES[token] ?? [404, { ok: false, description: "Not Found" }]);
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

test("an error body that echoes the request URL reaches stderr with the token redacted", async () => {
  const { status, stdout, stderr } = await send("tok-echo");
  assert.equal(status, 1);
  assert.equal(stdout, "");
  assert.match(stderr, /tg-send: HTTP 502: Bad Gateway: no upstream for \/bot<redacted>\/sendMessage/);
  assert.doesNotMatch(stderr, /tok-echo/);
});

test("an unreachable API exits 1 with an empty stdout", async () => {
  const { status, stdout, stderr } = await send("tok-ok", { api: "http://127.0.0.1:1" });
  assert.equal(status, 1);
  assert.equal(stdout, "");
  assert.match(stderr, /^tg-send: /);
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

// Rendering. Agents write the one markdown dialect CLAUDE.md mandates, and
// Telegram speaks HTML, so until tg-send translated, every summary reached the
// operator wearing its own asterisks (operator report, 2026-09-13). These pin
// the subset and, more importantly, the two ways the naive patterns misfire.

test("house markdown becomes Telegram's own markup", async () => {
  const { status } = await send("tok-ok", {
    body: "## Summary\n\n- **done**: shipped _clean_ via `x check`",
  });
  assert.equal(status, 0);
  assert.equal(
    seen.at(-1).text,
    "<b>Summary</b>\n\n• <b>done</b>: shipped <i>clean</i> via <code>x check</code>",
  );
});

// The final response run-notice pipes through here has no writer left to
// rewrite it, so the em dash CLAUDE.md forbids is swapped here, and only here;
// everywhere a writer is still present, deny-em-dash makes them do it.
test("an em dash becomes the comma the house rule names first", async () => {
  const { status } = await send("tok-ok", {
    body: "shipped \u2014 verified\u2014twice\n\u2014 trailing",
  });
  assert.equal(status, 0);
  assert.equal(seen.at(-1).text, "shipped, verified, twice\ntrailing");
});

test("a code span is literal, markup and refs inside it included", async () => {
  const { status } = await send("tok-ok", { body: "`**x**` and `#1` stay literal" });
  assert.equal(status, 0);
  assert.equal(
    seen.at(-1).text,
    "<code>**x**</code> and <code>#1</code> stay literal",
  );
});

// The budget footer tg-send carries every week is `seven_day: ... 44% left`.
// An italic rule that ignores word boundaries runs from that underscore to
// whichever one comes next and italicizes the paragraph between them.
test("underscores inside identifiers are not emphasis", async () => {
  const { status } = await send("tok-ok", {
    body: "seven_day: 56% used, 44% left, five_hour resets soon",
  });
  assert.equal(status, 0);
  assert.equal(
    seen.at(-1).text,
    "seven_day: 56% used, 44% left, five_hour resets soon",
  );
});

// Escaping runs before rendering, so a body cannot smuggle markup through.
test("markup characters in the body stay text", async () => {
  const { status } = await send("tok-ok", { body: "5 < 6 <b>not a tag</b>" });
  assert.equal(status, 0);
  assert.equal(seen.at(-1).text, "5 &lt; 6 &lt;b&gt;not a tag&lt;/b&gt;");
});

// The scrubber's marker is three asterisks a side and the bold rule is two,
// so a redacted body reached the operator as `*<b>REDACTED</b>*` (#598): the
// secret was gone, the message looked broken. The marker is matched whole,
// ahead of bold, and travels as typed. The token is assembled, never spelled,
// so no scanner reads this file as a leak.
test("the redaction marker is literal text, not a bold run", async () => {
  const { status, stderr } = await send("tok-ok", {
    body: "token ghp_" + "A1".repeat(12) + " leaked",
  });
  assert.equal(status, 0);
  assert.equal(seen.at(-1).text, "token ***REDACTED*** leaked");
  assert.match(stderr, /redacted 1 credential-shaped/);
});
