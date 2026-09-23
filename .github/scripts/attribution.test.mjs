// Fixtures for the attribution-bypass predicate (issue #510, widened to PR
// threads by #590). The whole mechanism keys on one structured field, the
// comment author's login, so these pin the predicate's edges: the good-path
// author is exempt, humans are exempt, a PR thread is no longer exempt, and
// a malformed user object must not crash the retrospect it rides in.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import json, sys
sys.path.insert(0, sys.argv[1])
import attribution
comments = json.loads(sys.argv[2])
print(json.dumps([c["id"] for c in attribution.flagged(comments)]))
`;

const comment = (id, login, issue) => ({
  id,
  user: login === null ? null : { login },
  issue_url: `https://api.github.com/repos/o/r/issues/${issue}`,
});

const fixtures = [
  {
    name: "claude[bot] on an issue is flagged",
    comments: [comment(1, "claude[bot]", 411)],
    expect: [1],
  },
  {
    name: "claude[bot] on a PR thread is flagged too: PR comments ride gh-comment since #590",
    comments: [comment(2, "claude[bot]", 513)],
    expect: [2],
  },
  {
    name: "github-actions[bot] is the good path, never flagged",
    comments: [comment(3, "github-actions[bot]", 411)],
    expect: [],
  },
  {
    name: "a human comment is not this module's business",
    comments: [comment(4, "bugabinga", 411)],
    expect: [],
  },
  {
    name: "a null user does not crash the predicate",
    comments: [comment(5, null, 411)],
    expect: [],
  },
  {
    name: "mixed window flags only the bypasses",
    comments: [
      comment(6, "github-actions[bot]", 411),
      comment(7, "claude[bot]", 513),
      comment(8, "claude[bot]", 411),
      comment(9, "bugabinga", 3),
    ],
    expect: [7, 8],
  },
];

test("attribution bypass predicate fixtures", async (t) => {
  for (const fixture of fixtures) {
    await t.test(fixture.name, () => {
      const result = spawnSync(
        "python3",
        ["-c", driver, scriptsDir, JSON.stringify(fixture.comments)],
        { encoding: "utf8" },
      );
      assert.equal(result.status, 0, result.stderr || result.stdout);
      assert.deepEqual(JSON.parse(result.stdout), fixture.expect);
    });
  }
});
