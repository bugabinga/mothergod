// seat-bun's contract: every workflow that runs anthropics/claude-code-action
// restores Bun through ./.github/actions/seat-bun, before the action, and
// hands the action that path, so no seat's wake downloads from GitHub's
// release CDN (#1961: six runs dead at `bun: command not found` in one hour
// of 504s). A seat copied from a neighbour without the step is red here,
// not on the next CDN fault; a step placed after the action leaves the
// output empty and the action installing its own Bun, silently.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const root = new URL("../..", import.meta.url).pathname;
const workflows = join(root, ".github/workflows");
const ACTION = "uses: anthropics/claude-code-action";
const STEP = "      - id: bun\n        uses: ./.github/actions/seat-bun\n";
const INPUT = "          path_to_bun_executable: ${{ steps.bun.outputs.path }}\n";

const seats = readdirSync(workflows)
  .filter((f) => f.endsWith(".yml"))
  .map((f) => [f, readFileSync(join(workflows, f), "utf8")])
  .filter(([, text]) => text.includes(ACTION));

test("every claude-code-action seat restores Bun through seat-bun, before the action", () => {
  assert.ok(seats.length > 0, "no seat found; the marker this test greps for is stale");
  for (const [f, text] of seats) {
    const step = text.indexOf(STEP);
    assert.notEqual(step, -1, `${f}: no seat-bun step`);
    assert.ok(step < text.indexOf(ACTION), `${f}: seat-bun runs after the action, so its output is empty`);
    assert.ok(text.includes(INPUT), `${f}: the action is not handed seat-bun's path`);
  }
});

test("seat-bun pins one version with a full checksum and keys the cache on it", () => {
  const action = readFileSync(join(root, ".github/actions/seat-bun/action.yml"), "utf8");
  assert.match(action, /^    default: "\d+\.\d+\.\d+"$/m, "version default is not one release");
  assert.match(action, /^    default: [0-9a-f]{64}$/m, "sha256 default is not 64 hex");
  assert.ok(action.includes("key: seat-bun-${{ inputs.version }}-linux-x64"), "cache key is not keyed on the version");
});
