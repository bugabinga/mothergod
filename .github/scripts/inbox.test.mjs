// Fixtures for inbox, the drain that used to be a hand-rolled curl.
//
// The first two assertions are the ones the script exists for: a KV read that
// cannot happen must exit non-zero and must never put `pending 0` on stdout.
// An inbox that reports empty when it is actually unreadable does not delay
// the operator's message, it drops it: the run reads "no messages", falls
// through to its scheduled duties, and posts a status line about something
// else while somebody waits at a phone. Nothing downstream records the miss.
//
// The renderers are pure by construction (no network, no clock: every input is
// an argument) precisely so these can exist. A detector nobody can make fire
// is decoration.
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;
const script = `${scriptsDir}/inbox`;

const driver = `
import importlib.machinery, importlib.util, json, sys
sys.path.insert(0, sys.argv[1])
loader = importlib.machinery.SourceFileLoader("inbox", sys.argv[1] + "/inbox")
spec = importlib.util.spec_from_loader("inbox", loader)
mod = importlib.util.module_from_spec(spec)
loader.exec_module(mod)
fn, raw = sys.argv[2], json.loads(sys.argv[3])
if fn == "render":
    print(json.dumps(mod.render([(k, v) for k, v in raw])))
elif fn == "render_chatlog":
    print(json.dumps(mod.render_chatlog(raw["log"], raw["keep"])))
elif fn == "body_of":
    print(json.dumps(mod.body_of(raw)))
elif fn == "when":
    print(json.dumps(mod.when(raw)))
elif fn == "one_line":
    print(json.dumps(mod.one_line(raw)))
elif fn == "turns":
    print(json.dumps(mod.turns(raw)))
elif fn == "call":
    try:
        print(json.dumps({"ok": mod.call(raw["url"], envelope=raw["envelope"])}))
    except mod.Unreadable as error:
        print(json.dumps({"unreadable": str(error)}))
`;

function py(fn, payload, env = {}) {
  const run = spawnSync(
    "python3",
    ["-c", driver, scriptsDir, fn, JSON.stringify(payload)],
    { encoding: "utf8", env: { ...process.env, ...env } },
  );
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

// spawn, never spawnSync, for anything the stub server must answer: the stub
// shares this event loop, and a spawnSync blocks it until the request it is
// waiting for times out. The same trap tg-send.test.mjs documents.
function pyAsync(fn, payload, env = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(
      "python3",
      ["-c", driver, scriptsDir, fn, JSON.stringify(payload)],
      { env: { ...process.env, ...env } },
    );
    let out = "";
    let err = "";
    child.stdout.on("data", (chunk) => (out += chunk));
    child.stderr.on("data", (chunk) => (err += chunk));
    child.on("close", (code) => code === 0 ? resolve(JSON.parse(out)) : reject(new Error(err)));
  });
}

function cli(args, env = {}) {
  return spawnSync(script, args, {
    encoding: "utf8",
    env: { ...process.env, ...env },
  });
}

// The CLI form of pyAsync, for the verbs a stub server has to answer.
function cliAsync(args, env = {}) {
  return new Promise((resolve) => {
    const child = spawn(script, args, { env: { ...process.env, ...env } });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (chunk) => (stdout += chunk));
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.on("close", (status) => resolve({ status, stdout, stderr }));
  });
}

// A stub Cloudflare, addressed through KV_API_BASE. Returns the env an
// `inbox` child needs to talk to it instead of the real API.
async function stubKV(handler) {
  const server = createServer(handler);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return {
    env: {
      KV_API_BASE: `http://127.0.0.1:${server.address().port}`,
      KV_ACCOUNT: "acct",
      KV_NAMESPACE: "ns",
      CLOUDFLARE_API_TOKEN: "planted-token",
    },
    close: () => server.close(),
  };
}

// Credentials absent is the cheapest way to reach the same branch a 403 or a
// renamed namespace reaches, and it needs no network at all.
const noCreds = { KV_ACCOUNT: "", KV_NAMESPACE: "", CLOUDFLARE_API_TOKEN: "" };

test("an unreadable inbox exits non-zero and never prints pending 0", () => {
  const run = cli(["drain"], noCreds);
  assert.notEqual(run.status, 0, "an unreadable drain must not exit 0");
  assert.match(run.stderr, /UNREADABLE, which is not the same as empty/);
  assert.doesNotMatch(run.stdout, /pending 0/);
});

test("an unreadable chatlog fails the same way", () => {
  const run = cli(["chatlog"], noCreds);
  assert.notEqual(run.status, 0);
  assert.match(run.stderr, /UNREADABLE/);
});

test("a genuinely empty inbox is pending 0, and asks for nothing", () => {
  const out = py("render", []);
  assert.match(out, /inbox: pending 0/);
  assert.doesNotMatch(out, /inbox done/);
});

test("a drained update carries its key, its instant, and its text", () => {
  const out = py("render", [
    ["u:000033118155", { message: { message_id: 485, date: 1789672837, text: "Check out the env-vars page" } }],
    ["u:000033118156", { message: { message_id: 487, date: 1789675469, text: "second" } }],
  ]);
  assert.match(out, /u:000033118155 {2}2026-09-17T\d\d:\d\d:\d\dZ {2}message 485/);
  assert.match(out, /Check out the env-vars page/);
  assert.match(out, /inbox: pending 2/);
  assert.match(out, /inbox done <key>/);
});

test("a captionless document renders its file_id, not a blank line", () => {
  const body = py("body_of", {
    message_id: 487,
    document: { file_name: "hidden-knowledge.html", file_id: "BQACAgIAAxkB" },
  });
  assert.deepEqual(body, [
    "[document: hidden-knowledge.html | file_id BQACAgIAAxkB]",
  ]);
});

test("a photo takes the largest size, which is the one worth fetching", () => {
  const body = py("body_of", {
    caption: "look",
    photo: [{ file_id: "small" }, { file_id: "large" }],
  });
  assert.deepEqual(body, ["look", "[photo: photo | file_id large]"]);
});

test("a message with nothing in it says so rather than rendering empty", () => {
  assert.deepEqual(py("body_of", { message_id: 1 }), ["[empty message]"]);
});

test("chatlog marks which turns this run already spoke", () => {
  const out = py("render_chatlog", {
    log: [
      { from: "operator", date: 1789672837, text: "first" },
      { from: "bdfl", date: 1789674643, text: "answered", run_id: "35264318181" },
    ],
    keep: 12,
  });
  assert.match(out, /operator/);
  assert.match(out, /run 35264318181/);
  assert.match(out, /chatlog 2 of 2 turns/);
});

test("chatlog -n keeps the tail, and a bad -n falls back to the default", () => {
  const log = {
    log: Array.from({ length: 5 }, (_, i) => ({ from: "bdfl", date: 1789672837, text: `t${i}` })),
    keep: 2,
  };
  const out = py("render_chatlog", log);
  assert.match(out, /chatlog 2 of 5 turns/);
  assert.match(out, /t4/);
  assert.doesNotMatch(out, /t2\b/);
  assert.equal(py("turns", ["-n", "3"]), 3);
  assert.equal(py("turns", ["-n", "banana"]), 12);
  assert.equal(py("turns", []), 12);
});

test("a multi-line turn is flattened, so one turn stays one line", () => {
  const out = py("render_chatlog", {
    log: [{ from: "operator", date: 1789672837, text: "first line\n\nsecond line" }],
    keep: 12,
  });
  const turnLines = out.split("\n").filter((l) => l && !l.startsWith("inbox:"));
  assert.equal(turnLines.length, 1, "a multi-line turn must still render as one line");
  assert.match(out, /first line second line/);
  assert.equal(py("one_line", ""), "[no text: attachment or empty message]");
  assert.match(py("one_line", "x".repeat(300)), /\u2026$/);
});

test("a nonsense timestamp renders as unknown instead of throwing", () => {
  assert.match(py("when", null), /\?\?\?\?-\?\?-\?\?/);
});

test("done refuses a key that is not an update, so chatlog cannot be deleted", () => {
  const run = cli(["done", "chatlog"]);
  assert.equal(run.status, 2);
  assert.match(run.stderr, /not an update key/);
});

test("done needs exactly one key", () => {
  const run = cli(["done"]);
  assert.equal(run.status, 2);
  assert.match(run.stderr, /exactly one key/);
});

test("an unknown verb names the usage that would have worked", () => {
  const run = cli(["slurp"]);
  assert.equal(run.status, 2);
  assert.match(run.stderr, /drain \| done <key> \| chatlog/);
});

test("done on a key another run already drained is success, not a red run", async () => {
  const stub = await stubKV((request, response) => {
    assert.equal(request.method, "DELETE");
    response.writeHead(404, { "content-type": "application/json" });
    response.end(JSON.stringify({ success: false, errors: [{ code: 10009 }] }));
  });
  try {
    const run = await cliAsync(["done", "u:000000000042"], stub.env);
    assert.equal(run.status, 0, "the losing side of the race did the work too");
    assert.match(run.stdout, /already gone/);
    assert.equal(run.stderr, "");
  } finally {
    stub.close();
  }
});

test("done on a key that genuinely failed to delete stays red", async () => {
  const stub = await stubKV((_request, response) => {
    response.writeHead(500, "Internal Server Error");
    response.end("boom");
  });
  try {
    const run = await cliAsync(["done", "u:000000000042"], stub.env);
    assert.equal(run.status, 1);
    assert.match(run.stderr, /NOT deleted/);
    assert.doesNotMatch(run.stderr, /planted-token/);
  } finally {
    stub.close();
  }
});

test("done deletes, and says the reply was the receipt", async () => {
  const stub = await stubKV((_request, response) => {
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({ success: true, result: null }));
  });
  try {
    const run = await cliAsync(["done", "u:000000000042"], stub.env);
    assert.equal(run.status, 0);
    assert.match(run.stdout, /deleted \| the reply was the receipt/);
  } finally {
    stub.close();
  }
});

test("a listed key that vanishes before the read does not fail the drain", async () => {
  const stub = await stubKV((request, response) => {
    if (request.url.includes("/keys")) {
      response.writeHead(200, { "content-type": "application/json" });
      response.end(JSON.stringify({ success: true, result: [{ name: "u:000000000042" }] }));
      return;
    }
    response.writeHead(404, { "content-type": "application/json" });
    response.end(JSON.stringify({ success: false, errors: [] }));
  });
  try {
    const run = await cliAsync(["drain"], stub.env);
    assert.equal(run.status, 0);
    assert.match(run.stdout, /\[empty message\]/);
    assert.match(run.stdout, /pending 1/);
  } finally {
    stub.close();
  }
});

test("a 403 from Cloudflare is Unreadable, and the token is not in the text", async () => {
  const server = createServer((_request, response) => {
    response.writeHead(403, "Forbidden");
    response.end("nope");
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const url = `http://127.0.0.1:${server.address().port}/keys`;
  try {
    const out = await pyAsync("call", { url, envelope: true }, { CLOUDFLARE_API_TOKEN: "s3cret-token" });
    assert.match(out.unreadable, /403/);
    assert.doesNotMatch(out.unreadable, /s3cret-token/);
    assert.equal(out.ok, undefined, "a 403 must not decode as a result");
  } finally {
    server.close();
  }
});

test("a success:false envelope is Unreadable even though the HTTP call worked", async () => {
  const server = createServer((_request, response) => {
    response.writeHead(200, { "content-type": "application/json" });
    response.end(JSON.stringify({ success: false, errors: [{ code: 10001 }], result: [] }));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const url = `http://127.0.0.1:${server.address().port}/keys`;
  try {
    const out = await pyAsync("call", { url, envelope: true }, { CLOUDFLARE_API_TOKEN: "unused-token" });
    assert.match(out.unreadable, /refused/);
  } finally {
    server.close();
  }
});
