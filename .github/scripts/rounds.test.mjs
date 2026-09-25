// Fixtures for the rounds line, #663's number. The comment bodies are the
// shape gh-comment writes: prose, then the role footer on the last line.
// The expectations pin the docstring's edges: only the reviewer's footer
// counts, only on the last line, a PR with no reviewer post is the 0
// bucket, three or more is the tail, and a full page says it is capped.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { test } from "node:test";

const scriptsDir = new URL(".", import.meta.url).pathname;

const driver = `
import io, json, sys
sys.path.insert(0, sys.argv[1])
import rounds
prs = json.loads(sys.argv[2])
limit = int(sys.argv[3]) if len(sys.argv) > 3 else rounds.LIMIT
buf = io.StringIO()
n = rounds.render(prs, out=buf, limit=limit)
print(json.dumps({"n": n, "line": buf.getvalue().rstrip("\\n")}))
`;

const post = (role, prose = "Verdict.") =>
  ({ body: `${prose}\n\n---\n_${role} · [Claude Code](https://claude.ai/code)_` });
const pr = (number, ...comments) => ({ number, mergedAt: "2026-09-25T08:00:00Z", comments });

const fixtures = [
  {
    name: "an empty window says so",
    prs: [],
    expect: { n: 0, line: "rounds: merged 0 since the anchor" },
  },
  {
    name: "the 2026-09-25 morning: one, two and four posts, other seats ignored",
    prs: [
      pr(760, post("reviewer")),
      pr(759, post("reviewer"), post("reviewer")),
      pr(750, post("reviewer"), post("maintainer"), post("reviewer"), post("reviewer")),
      pr(755, post("reviewer"), post("reviewer"), post("reviewer"), post("reviewer")),
    ],
    expect: {
      n: 4,
      line: "rounds: merged 4 | posts per PR 0:0 1:1 2:1 3+:2 | tail #755 (4) #750 (3)",
    },
  },
  {
    name: "a PR with no comments, and one with only the BDFL's, are the 0 bucket",
    prs: [pr(700), pr(701, post("bdfl")), pr(702, post("reviewer"))],
    expect: { n: 3, line: "rounds: merged 3 | posts per PR 0:2 1:1 2:0 3+:0" },
  },
  {
    name: "a reviewer footer quoted mid-body is not a reviewer post",
    prs: [
      pr(
        703,
        { body: "The reviewer wrote:\n\n> _reviewer · [Claude Code](https://claude.ai/code)_\n\nand I disagree.\n\n---\n_bdfl · [Claude Code](https://claude.ai/code)_" },
        post("reviewer"),
      ),
    ],
    expect: { n: 1, line: "rounds: merged 1 | posts per PR 0:0 1:1 2:0 3+:0" },
  },
  {
    name: "a full page is reported as capped, never as the whole window",
    prs: [pr(1, post("reviewer")), pr(2, post("reviewer"))],
    limit: 2,
    expect: {
      n: 2,
      line: "rounds: merged 2 | posts per PR 0:0 1:2 2:0 3+:0 | capped at 2, the oldest merges are not counted",
    },
  },
];

test("rounds line fixtures", async (t) => {
  for (const fixture of fixtures) {
    await t.test(fixture.name, () => {
      const result = spawnSync(
        "python3",
        ["-c", driver, scriptsDir, JSON.stringify(fixture.prs), ...(fixture.limit ? [String(fixture.limit)] : [])],
        { encoding: "utf8" },
      );
      assert.equal(result.status, 0, result.stderr || result.stdout);
      assert.deepEqual(JSON.parse(result.stdout), fixture.expect);
    });
  }
});
