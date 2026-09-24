// tg-draft is decoration with one hard contract: exit 0 whatever happens,
// and when it posts, post the right text under the worker's secret. A local
// server stands in for the webhook worker over MOTHERGOD_WORKER_BASE.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";

const script = new URL("tg-draft", import.meta.url).pathname;
const SECRET = "worker-secret-for-tests";

const posts = [];
const server = createServer((req, res) => {
  let data = "";
  req.on("data", (chunk) => (data += chunk));
  req.on("end", () => {
    posts.push({ path: req.url, secret: req.headers["x-mothergod-secret"], body: JSON.parse(data) });
    res.writeHead(204);
    res.end();
  });
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const base = `http://127.0.0.1:${server.address().port}`;
after(() => server.close());

// One temp directory for the whole file: sessions are told apart by id, and
// a test that needs a fresh run picks a fresh session_id.
const TMPDIR = mkdtempSync(join(tmpdir(), "tg-draft-"));

// spawn, never spawnSync: the stub answers on this same event loop.
function hook(call, env = {}) {
  return new Promise((resolve) => {
    const child = spawn("python3", [script], {
      env: {
        PATH: process.env.PATH,
        TMPDIR,
        GITHUB_ACTIONS: "true",
        GITHUB_WORKFLOW: "agent-bdfl",
        TELEGRAM_WEBHOOK_SECRET: SECRET,
        MOTHERGOD_WORKER_BASE: base,
        ...env,
      },
    });
    let stderr = "";
    child.stderr.setEncoding("utf8").on("data", (chunk) => (stderr += chunk));
    child.on("close", (status) => resolve({ status, stderr }));
    child.stdin.end(typeof call === "string" ? call : JSON.stringify(call));
  });
}

function bash(session_id, description) {
  return { session_id, tool_name: "Bash", tool_input: { command: "true", description } };
}

test("a Bash call's description reaches the worker under the seat's name", async () => {
  const before = posts.length;
  const { status } = await hook(bash("s-one", "Run the worker tests"));
  assert.equal(status, 0);
  const post = posts[before];
  assert.equal(post.path, "/typing/draft");
  assert.equal(post.secret, SECRET);
  assert.equal(post.body.text, "bdfl, starting\n· Run the worker tests");
});

test("the draft keeps the last four calls of a session, newest last", async () => {
  for (const step of ["one", "two", "three", "four", "five"]) {
    await hook(bash("s-two", `step ${step}`));
  }
  assert.equal(
    posts.at(-1).body.text,
    "bdfl, starting\n· step two\n· step three\n· step four\n· step five",
  );
});

test("edits name the file relative to the checkout; fetches name the host", async () => {
  await hook({
    session_id: "s-three",
    cwd: "/w/repo",
    tool_name: "Edit",
    tool_input: { file_path: "/w/repo/src/lib.rs", old_string: "a", new_string: "b" },
  });
  await hook({
    session_id: "s-three",
    tool_name: "WebFetch",
    tool_input: { url: "https://core.telegram.org/bots/api", prompt: "..." },
  });
  await hook({ session_id: "s-three", tool_name: "Skill", tool_input: { skill: "adr" } });
  assert.equal(posts.at(-1).body.text, "bdfl, starting\n· edit src/lib.rs\n· fetch core.telegram.org\n· skill adr");
});

test("a tool with no dedicated words is spaced out, not squashed", async () => {
  await hook({ session_id: "s-eight", tool_name: "ListAgents", tool_input: {} });
  await hook({ session_id: "s-eight", tool_name: "ScheduleWakeup", tool_input: {} });
  await hook({
    session_id: "s-eight",
    tool_name: "mcp__github_file_ops__commit_files",
    tool_input: {},
  });
  assert.equal(
    posts.at(-1).body.text,
    "bdfl, starting\n· list agents\n· schedule wakeup\n· commit files",
  );
});

test("a credential in a description is redacted before it leaves the runner", async () => {
  await hook(bash("s-four", "push with ghp_abcdefghijklmnopqrstuvwxyz0123456789"));
  const text = posts.at(-1).body.text;
  assert.ok(text.includes("***REDACTED***"), text);
  assert.ok(!text.includes("ghp_"), text);
});

test("a description longer than a phone line is cut, not wrapped", async () => {
  await hook(bash("s-five", "x".repeat(200)));
  const line = posts.at(-1).body.text.split("\n")[1];
  assert.equal(line.length, 2 + 90, line);
  assert.ok(line.endsWith("…"));
});

test("outside Actions, without the secret, or on malformed input nothing is posted, exit 0", async () => {
  const before = posts.length;
  const runs = await Promise.all([
    hook(bash("s-six", "a"), { GITHUB_ACTIONS: "" }),
    hook(bash("s-six", "b"), { TELEGRAM_WEBHOOK_SECRET: "" }),
    hook("not json"),
    hook({ session_id: "s-six", tool_input: {} }),
  ]);
  for (const { status } of runs) assert.equal(status, 0);
  assert.equal(posts.length, before);
});

test("a dead worker costs a stderr line, never a non-zero exit, and never the secret", async () => {
  const { status, stderr } = await hook(bash("s-seven", "a"), {
    MOTHERGOD_WORKER_BASE: "http://127.0.0.1:1",
  });
  assert.equal(status, 0);
  assert.match(stderr, /tg-draft: not delivered/);
  assert.ok(!stderr.includes(SECRET));
});
