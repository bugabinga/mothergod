// Fixtures for secret_scrub, the redactor that guards every exit from a run.
//
// The test this replaces is the reason the module exists. Issue #425 added
// prefix patterns to catch an app installation token echoed by `git remote -v`,
// and pinned them with a synthetic `ghs_`-shaped fake. On 2026-09-18 the leak
// recurred and the patterns matched nothing: the runner had begun minting a
// 390-character token with `_` and `-`, and every pattern wanted the 40-char
// `ghs_` form. The test stayed green the whole time, because it only ever
// proved the regex matched its own fixture.
//
// So the assertions below are shaped against reality, not against the fixture:
// the long modern token, the live `remote.origin.url` of the checkout this test
// runs in, and the false-positive cases that decide whether the userinfo rule
// is safe to run over ordinary prose.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

// Fakes are built by concatenation so no token-shaped literal lives in the
// repo to trip secret scanning.
const longToken = "A1_b2-" + "Xy9_zQ4-".repeat(48);
const ghsToken = "ghs_" + "A1".repeat(18);
const patToken = "github_pat_" + "B2".repeat(15);
const antToken = "sk-ant-" + "c3".repeat(15);
const tgToken = "123456789:" + "Dd_-".repeat(9);

function scrub(text, values = []) {
  const driver = `
import json, sys
sys.path.insert(0, sys.argv[1])
from secret_scrub import scrub
text, values = json.loads(sys.argv[2]), json.loads(sys.argv[3])
out, count = scrub(text, values)
print(json.dumps({"out": out, "count": count}))
`;
  const result = spawnSync(
    "python3",
    ["-c", driver, scriptsDir, JSON.stringify(text), JSON.stringify(values)],
    { encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

// The regression that motivated the module: length- and format-independent.
test("a modern long-format token in a remote URL is redacted", () => {
  const { out, count } = scrub(
    `origin\thttps://x-access-token:${longToken}@github.com/bugabinga/mothergod.git (fetch)`,
  );
  assert.ok(!out.includes(longToken), "the 390-char token survived");
  assert.ok(!out.includes("x-access-token"), "userinfo survived");
  assert.equal(count, 1);
  assert.ok(out.includes("https://***REDACTED***@github.com/"), out);
});

// Liveness (the property the old test lacked): whatever this runner actually
// put in .git/config, the scrubber must leave nothing credential-like behind.
// If a future runner invents another shape, this goes red instead of rotting.
test("the live remote URL of this checkout carries no credential past scrub", () => {
  const url = execFileSync("git", ["config", "--get", "remote.origin.url"], {
    encoding: "utf8",
  }).trim();
  const { out } = scrub(url);
  // Any userinfo left standing must BE the marker; the marker itself contains
  // no `/`, space or `@`, so a naive "no userinfo" probe would match it and
  // pass on a live token just as readily.
  const userinfo = out.match(/\/\/([^/\s@]+)@/);
  if (userinfo) {
    assert.equal(userinfo[1], "***REDACTED***", "a live credential survived in the remote URL");
  }
  if (/\/\/[^/\s@]+@/.test(url)) {
    assert.ok(out.includes("***REDACTED***"), "a credentialed remote was not redacted");
  }
});

// The runner hands git its credential base64-encoded (#738), so every prefix
// rule sees an opaque blob: the header shape is the only handle.
test("the Authorization header the runner writes to git config is redacted", () => {
  const b64 = Buffer.from(`x-access-token:${longToken}`).toString("base64");
  const { out, count } = scrub(
    `http.https://github.com/.extraheader=AUTHORIZATION: basic ${b64}`,
  );
  assert.ok(!out.includes(b64), "the base64 credential survived");
  assert.equal(count, 1);
  assert.ok(out.includes("AUTHORIZATION: basic ***REDACTED***"), out);
  for (const scheme of ["Bearer", "token"]) {
    const { out } = scrub(`curl -H "Authorization: ${scheme} ${patToken}"`);
    assert.ok(!out.includes(patToken), `${scheme} value survived`);
    assert.ok(out.includes(`Authorization: ${scheme} ***REDACTED***`), out);
  }
});

// Liveness, as for the remote URL: whatever header this runner actually put in
// its git config, nothing credential-shaped survives the scrub.
test("the live extraheader of this checkout carries no credential past scrub", () => {
  const result = spawnSync(
    "git",
    ["config", "--get-all", "http.https://github.com/.extraheader"],
    { encoding: "utf8" },
  );
  for (const header of result.stdout.split("\n").map((s) => s.trim()).filter(Boolean)) {
    const { out } = scrub(header);
    const value = out.match(/^authorization:\s*\S+\s+(\S+)/i);
    assert.ok(value, `unrecognised header shape: ${out.slice(0, 24)}`);
    assert.equal(value[1], "***REDACTED***", "a live credential survived in the extraheader");
  }
});

test("bare prefixed tokens outside a URL are still caught", () => {
  for (const fake of [ghsToken, patToken, antToken, tgToken]) {
    const { out } = scrub(`saw ${fake} in the environment`);
    assert.ok(!out.includes(fake), `leaked ${fake.slice(0, 12)}...`);
  }
});

test("the Telegram /bot<token>/ URL form is caught", () => {
  const { out } = scrub(`called https://api.telegram.org/bot${tgToken}/sendMessage`);
  assert.ok(!out.includes(tgToken), "telegram token survived");
});

test("explicit values are redacted, and short ones are ignored", () => {
  const { out } = scrub("cloudflare=bare0alnum0secret and x=ab", ["bare0alnum0secret", "ab"]);
  assert.ok(!out.includes("bare0alnum0secret"), "named secret survived");
  assert.ok(out.includes("x=ab"), "a 2-char value must not be redacted; it would erase prose");
});

// The userinfo rule runs over every issue comment this project posts, so its
// false-positive behaviour is load-bearing, not a nicety.
test("ordinary prose and links are left alone", () => {
  const prose = [
    "see https://github.com/bugabinga/mothergod/issues/425 for the history",
    "mail a@b.com or read https://mothergod.dev/status.html",
    "the path https://example.com/a/user@example is not userinfo",
    "authorization: token holders decide, per ADR-0011",
    "`cargo x check` runs fmt, lint, test, doc",
  ].join("\n");
  const { out, count } = scrub(prose);
  assert.equal(count, 0, `false positive: ${out}`);
  assert.equal(out, prose);
});
