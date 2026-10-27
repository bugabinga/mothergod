// ci.yml's `changes` job runs `test` on a PR only for a changed path its
// `rust` filter names, and tests/claims.rs reads files that filter has to
// list by hand: a claim about an unlisted file passes every PR and fails on
// main's next push, where every job runs (#1009; LICENSE arrived that way
// in #1215, as a binary argument no scan of `read(` calls would see).
// claims.rs declares its inputs in one `INPUTS` const and checks every read
// and binary argument against it at runtime; this test holds the filter to
// that const, so an entry the regex misses fails `worker` in the same PR.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const repoRoot = new URL("../..", import.meta.url).pathname;

// The entries of claims.rs's `INPUTS` const, in source order.
function claimsInputs(source) {
  const block = source.match(/const INPUTS: \[&str; \d+\] = \[([^\]]*)\];/);
  assert.ok(block, "tests/claims.rs declares `const INPUTS`");
  return [...block[1].matchAll(/"([^"]+)"/g)].map((entry) => entry[1]);
}

// The `grep -qE` pattern that sets `rust=true` in ci.yml's `changes` job,
// as a RegExp: grep tests one path per line, so `^` and `$` anchor a whole
// path here too.
function rustFilter(ci) {
  const lines = ci.split("\n");
  const guard = lines.find(
    (line, i) => line.includes("grep -qE '") && lines[i + 1]?.includes("echo \"rust=true\""),
  );
  assert.ok(guard, "ci.yml's changes job sets rust=true under a grep -qE guard");
  return new RegExp(guard.match(/grep -qE '([^']*)'/)[1]);
}

// The entries `filter` does not cover. A directory entry is probed with a
// file that has no extension, so the directory itself has to be named.
function misses(inputs, filter) {
  return inputs.filter((entry) => !filter.test(entry.endsWith("/") ? `${entry}new-file` : entry));
}

test("misses names the entries a filter skips, directories probed by prefix", () => {
  const filter = /\.rs$|^site\/|^README\.md$/;
  assert.deepEqual(misses(["README.md", "site/", "docs/benchmarks/", "LICENSE"], filter), [
    "docs/benchmarks/",
    "LICENSE",
  ]);
});

test("the extracted filter is the rust one, not quality", () => {
  const filter = rustFilter(readFileSync(`${repoRoot}/.github/workflows/ci.yml`, "utf8"));
  assert.match("src/lib.rs", filter);
  assert.doesNotMatch("agents/README.md", filter, "quality's `.md$` would match this");
});

test("every input tests/claims.rs reads matches ci.yml's rust filter", () => {
  const inputs = claimsInputs(readFileSync(`${repoRoot}/tests/claims.rs`, "utf8"));
  assert.ok(inputs.length > 0, "INPUTS is non-empty");
  const filter = rustFilter(readFileSync(`${repoRoot}/.github/workflows/ci.yml`, "utf8"));
  assert.deepEqual(
    misses(inputs, filter),
    [],
    "listed in tests/claims.rs INPUTS but ci.yml's rust filter skips a PR editing only it",
  );
});
