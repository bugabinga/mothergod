// Fixtures for the attribution-bypass predicate (issue #510). The whole
// mechanism keys on one structured field, the comment author's login, so
// these pin the predicate's edges: PR threads are exempt, the good-path
// author is exempt, humans are exempt, and a malformed user object must
// not crash the retrospect it rides in.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import json, sys
sys.path.insert(0, sys.argv[1])
import attribution
comments = json.loads(sys.argv[2])
prs = set(json.loads(sys.argv[3]))
print(json.dumps([c["id"] for c in attribution.flagged(comments, prs)]))
`;

const comment = (id, login, issue, overrides = {}) => ({
  id,
  user: login === null ? null : { login },
  issue_url: `https://api.github.com/repos/o/r/issues/${issue}`,
  ...overrides,
});

const fixtures = [
  {
    name: "claude[bot] on an issue is flagged",
    comments: [comment(1, "claude[bot]", 411)],
    prs: [],
    expect: [1],
  },
  {
    name: "claude[bot] on a PR thread is legitimate",
    comments: [comment(2, "claude[bot]", 513)],
    prs: [513],
    expect: [],
  },
  {
    name: "github-actions[bot] is the good path, never flagged",
    comments: [comment(3, "github-actions[bot]", 411)],
    prs: [],
    expect: [],
  },
  {
    name: "a human comment is not this module's business",
    comments: [comment(4, "bugabinga", 411)],
    prs: [],
    expect: [],
  },
  {
    name: "a null user does not crash the predicate",
    comments: [comment(5, null, 411)],
    prs: [],
    expect: [],
  },
  {
    name: "mixed window flags only the bypass",
    comments: [
      comment(6, "github-actions[bot]", 411),
      comment(7, "claude[bot]", 513),
      comment(8, "claude[bot]", 411),
      comment(9, "bugabinga", 3),
    ],
    prs: [513],
    expect: [8],
  },
];

test("attribution bypass predicate fixtures", async (t) => {
  for (const fixture of fixtures) {
    await t.test(fixture.name, () => {
      const result = spawnSync(
        "python3",
        [
          "-c",
          driver,
          scriptsDir,
          JSON.stringify(fixture.comments),
          JSON.stringify(fixture.prs),
        ],
        { encoding: "utf8" },
      );
      assert.equal(result.status, 0, result.stderr || result.stdout);
      assert.deepEqual(JSON.parse(result.stdout), fixture.expect);
    });
  }
});
