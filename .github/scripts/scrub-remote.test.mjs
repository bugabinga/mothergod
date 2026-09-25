// The hook's whole job is a side effect on .git/config, so every case builds
// a throwaway repository and reads the remote back through git itself. The
// token is a fake in the `ghs_` shape; the property under test is that no
// stream and no summary line ever carries it, whatever its shape.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

const script = new URL("scrub-remote", import.meta.url).pathname;
const token = `ghs_${"A".repeat(36)}`;
const dirty = `https://x-access-token:${token}@github.com/o/r.git`;
const clean = "https://github.com/o/r.git";

// What actions/checkout persists (git-auth-helper.ts): the workflow token,
// base64 of `x-access-token:<token>`, under the extraheader key.
const HEADER_KEY = "http.https://github.com/.extraheader";
const b64 = Buffer.from(`x-access-token:${token}`).toString("base64");
const header = `AUTHORIZATION: basic ${b64}`;

function repo(url, withHeader = false) {
  const dir = mkdtempSync(join(tmpdir(), "scrub-remote-"));
  execFileSync("git", ["-C", dir, "init", "-q"]);
  if (url) execFileSync("git", ["-C", dir, "remote", "add", "origin", url]);
  if (withHeader) execFileSync("git", ["-C", dir, "config", HEADER_KEY, header]);
  return dir;
}

function headers(dir) {
  const r = spawnSync("git", ["-C", dir, "config", "--get-all", HEADER_KEY], {
    encoding: "utf8",
  });
  return r.status === 0 ? r.stdout.trim() : "";
}

function origin(dir) {
  return execFileSync("git", ["-C", dir, "config", "--get", "remote.origin.url"], {
    encoding: "utf8",
  }).trim();
}

function run(dir, extraEnv = {}) {
  const summary = join(dir, "summary.md");
  writeFileSync(summary, "");
  const result = spawnSync(script, [], {
    input: JSON.stringify({ hook_event_name: "SessionStart", source: "startup" }),
    env: {
      PATH: process.env.PATH,
      GITHUB_ACTIONS: "true",
      CLAUDE_PROJECT_DIR: dir,
      GITHUB_STEP_SUMMARY: summary,
      ...extraEnv,
    },
    encoding: "utf8",
  });
  return { ...result, summary: readFileSync(summary, "utf8") };
}

test("a credentialed origin loses its userinfo, and no stream carries the token", () => {
  const dir = repo(dirty);
  const r = run(dir);
  assert.equal(r.status, 0);
  assert.equal(origin(dir), clean);
  for (const text of [r.stdout, r.stderr, r.summary]) {
    assert.ok(!text.includes(token), "token escaped");
    assert.ok(!text.includes("x-access-token"), "userinfo escaped");
  }
  assert.match(r.stdout, /origin credential removed/);
  assert.match(r.summary, /origin credential removed/);
});

test("the persisted extraheader is unset, and no stream carries its value", () => {
  const dir = repo(dirty, true);
  const r = run(dir);
  assert.equal(r.status, 0);
  assert.equal(origin(dir), clean);
  assert.equal(headers(dir), "", "the extraheader survived the hook");
  for (const text of [r.stdout, r.stderr, r.summary]) {
    assert.ok(!text.includes(b64), "header value escaped");
    assert.ok(!text.includes(token), "token escaped");
  }
  assert.match(r.stdout, /origin credential removed, extraheader removed/);
});

test("an already clean origin is left alone and named as the deletion signal", () => {
  const dir = repo(clean);
  const r = run(dir);
  assert.equal(r.status, 0);
  assert.equal(origin(dir), clean);
  assert.match(r.stdout, /origin already clean, extraheader already clean/);
  assert.match(r.summary, /already clean/);
});

test("an ssh origin has no http userinfo and is untouched", () => {
  const ssh = "git@github.com:o/r.git";
  const dir = repo(ssh);
  const r = run(dir);
  assert.equal(r.status, 0);
  assert.equal(origin(dir), ssh);
});

test("outside GitHub Actions the remote is not ours to touch", () => {
  const dir = repo(dirty);
  const r = run(dir, { GITHUB_ACTIONS: "" });
  assert.equal(r.status, 0);
  assert.equal(origin(dir), dirty);
  assert.equal(r.stdout, "");
  assert.equal(r.summary, "");
});

test("no origin says nothing about it; no repository exits 0 in silence", () => {
  const noRemote = run(repo(null));
  assert.equal(noRemote.status, 0);
  assert.equal(noRemote.stdout.trim(), "scrub-remote: extraheader already clean");
  const noRepo = run(mkdtempSync(join(tmpdir(), "scrub-remote-none-")));
  assert.equal(noRepo.status, 0);
  assert.equal(noRepo.stdout, "");
});
