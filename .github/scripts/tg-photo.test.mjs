// tg-photo's one job is putting before beside after in the operator's chat,
// so the multipart body is the contract: item order, the caption on the
// first item, every file attached. A local server stands in for Telegram
// over TELEGRAM_API_BASE; the token doubles as the secret that must never
// reach stderr, as in tg-send.test.mjs.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";

const script = new URL("tg-photo", import.meta.url).pathname;

const seen = [];
const server = createServer((req, res) => {
  const [, bot, method] = req.url.split("/");
  const token = bot.slice("bot".length);
  const chunks = [];
  req.on("data", (chunk) => chunks.push(chunk));
  req.on("end", () => {
    seen.push({ method, body: Buffer.concat(chunks).toString("latin1") });
    if (token === "tok-denied") {
      res.writeHead(400, { "content-type": "application/json" });
      res.end(JSON.stringify({ ok: false, description: `Bad Request: no chat for /bot${token}` }));
      return;
    }
    const result = method === "sendMediaGroup"
      ? [{ message_id: 700 }, { message_id: 701 }, { message_id: 702 }, { message_id: 703 }]
      : { message_id: 710 };
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ ok: true, result }));
  });
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const base = `http://127.0.0.1:${server.address().port}`;
after(() => server.close());

function shots(rows) {
  const dir = mkdtempSync(join(tmpdir(), "tg-photo-"));
  for (const row of rows) {
    for (const phase of ["before", "after"]) {
      if (row[phase]) writeFileSync(join(dir, row[phase]), `png:${row[phase]}`);
    }
  }
  writeFileSync(join(dir, "manifest.json"), JSON.stringify({ shots: rows }));
  return dir;
}

function send(token, dir) {
  return new Promise((resolve) => {
    const child = spawn("python3", [script, dir], {
      env: {
        PATH: process.env.PATH,
        TELEGRAM_API_BASE: base,
        TELEGRAM_BOT_TOKEN: token,
        OPERATOR_TELEGRAM_CHAT_ID: "42",
      },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8").on("data", (chunk) => (stdout += chunk));
    child.stderr.setEncoding("utf8").on("data", (chunk) => (stderr += chunk));
    child.on("close", (status) => resolve({ status, stdout, stderr }));
  });
}

const row = (viewport, pr = 12) => ({
  branch: "claude/herald-x",
  sha: "abc",
  base: "def",
  pr,
  page: "site/index.html",
  viewport,
  before: `index-${viewport}-before.png`,
  after: `index-${viewport}-after.png`,
});

test("one media group per page: before beside after, narrow then wide, caption first", async () => {
  seen.length = 0;
  const r = await send("tok-ok", shots([row("375x812"), row("1440x900")]));
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout, "message_ids=700,701,702,703\n");
  assert.equal(seen.length, 1);
  assert.equal(seen[0].method, "sendMediaGroup");
  const media = JSON.parse(seen[0].body.match(/name="media"\r\n\r\n(.*?)\r\n/s)[1]);
  assert.deepEqual(media.map((m) => m.media), ["attach://p0", "attach://p1", "attach://p2", "attach://p3"]);
  assert.match(media[0].caption, /pull\/12">#12<\/a> site\/index.html: before and after at 375x812, 1440x900/);
  assert.equal(media[1].caption, undefined);
  const files = [...seen[0].body.matchAll(/filename="([^"]+)"/g)].map((m) => m[1]);
  assert.deepEqual(files, [
    "index-375x812-before.png",
    "index-375x812-after.png",
    "index-1440x900-before.png",
    "index-1440x900-after.png",
  ]);
  assert.ok(seen[0].body.includes("png:index-375x812-before.png"));
});

test("a single still goes as one photo, captioned by branch when there is no PR", async () => {
  seen.length = 0;
  const only = { ...row("375x812", null), before: null };
  const r = await send("tok-ok", shots([only]));
  assert.equal(r.status, 0, r.stderr);
  assert.equal(r.stdout, "message_ids=710\n");
  assert.equal(seen[0].method, "sendPhoto");
  assert.match(seen[0].body, /<code>claude\/herald-x<\/code> site\/index.html/);
});

test("an empty manifest sends nothing and exits 0", async () => {
  seen.length = 0;
  const r = await send("tok-ok", shots([]));
  assert.equal(r.status, 0);
  assert.equal(r.stdout, "tg-photo: nothing to send\n");
  assert.equal(seen.length, 0);
});

test("a Telegram error is a non-zero exit with the description, token scrubbed", async () => {
  const r = await send("tok-denied", shots([row("375x812"), row("1440x900")]));
  assert.equal(r.status, 1);
  assert.equal(r.stdout, "");
  assert.match(r.stderr, /HTTP 400: Bad Request/);
  assert.ok(!r.stderr.includes("tok-denied"), r.stderr);
});
